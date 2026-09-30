#!/usr/bin/env node
// Local-only manifest construction. Reads public verification material, never keys.
import { createHash, createPublicKey, verify } from 'node:crypto';
import { readFileSync, statSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const MAX_BYTES = 128 * 1024 * 1024;
function base64(value) {
  if (typeof value !== 'string' || !value.length || value.length > 8192 || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value)) throw new Error('Invalid signature encoding.');
  const bytes = Buffer.from(value, 'base64');
  if (bytes.toString('base64') !== value) throw new Error('Invalid signature encoding.');
  return bytes;
}
export function createManifest({ version, url, artifact, signature, publicKey }) {
  // Stable releases only: no ambiguous v prefix, build metadata or prereleases.
  if (typeof version !== 'string' || version.length > 100 || !/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version)) throw new Error('Version must be a stable numeric semver.');
  if (version.split('.').some(part => BigInt(part) > 18446744073709551615n)) throw new Error('Version component exceeds semver limits.');
  let endpoint;
  try { endpoint = new URL(url); } catch { throw new Error('Invalid artifact URL.'); }
  if (typeof url !== 'string' || url.length > 4096 || /[\s\x00-\x1f]/.test(url) || endpoint.protocol !== 'https:' || !endpoint.hostname || endpoint.username || endpoint.password || endpoint.hash || endpoint.search || !endpoint.pathname.endsWith('.app.tar.gz')) throw new Error('Artifact URL must be HTTPS and end in .app.tar.gz without credentials, query or fragment.');
  if (!Buffer.isBuffer(artifact) || !artifact.length || artifact.length > MAX_BYTES || artifact[0] !== 0x1f || artifact[1] !== 0x8b) throw new Error('Artifact must be a nonempty bounded gzip updater archive.');
  const sig = signature.trim();
  const signatureLines = base64(sig).toString('utf8').trimEnd().split('\n');
  const keyLines = base64(publicKey.trim()).toString('utf8').trimEnd().split('\n');
  if (signatureLines.length !== 4 || !signatureLines[0].startsWith('untrusted comment: ') || !signatureLines[2].startsWith('trusted comment: ') || keyLines.length !== 2 || !keyLines[0].startsWith('untrusted comment: ')) throw new Error('Invalid Minisign structure.');
  const keyBytes = base64(keyLines[1]);
  const sigBytes = base64(signatureLines[1]);
  const globalSignature = base64(signatureLines[3]);
  if (keyBytes.length !== 42 || keyBytes.subarray(0, 2).toString() !== 'Ed' || sigBytes.length !== 74 || globalSignature.length !== 64 || !keyBytes.subarray(2, 10).equals(sigBytes.subarray(2, 10))) throw new Error('Invalid Minisign key or signature.');
  const algorithm = sigBytes.subarray(0, 2).toString();
  if (!['ED', 'Ed'].includes(algorithm)) throw new Error('Unsupported signature algorithm.');
  const key = createPublicKey({ key: Buffer.concat([Buffer.from('302a300506032b6570032100', 'hex'), keyBytes.subarray(10)]), format: 'der', type: 'spki' });
  const payload = algorithm === 'ED' ? createHash('blake2b512').update(artifact).digest() : artifact;
  const trusted = signatureLines[2].slice('trusted comment: '.length);
  if (!verify(null, payload, key, sigBytes.subarray(10)) || !verify(null, Buffer.concat([sigBytes.subarray(10), Buffer.from(trusted)]), key, globalSignature)) throw new Error('Artifact signature verification failed.');
  const versions = trusted.split('\t').filter(field => field.startsWith('version:')).map(field => field.slice(8));
  if (versions.length !== 1 || versions[0].replace(/^v/, '') !== version) throw new Error('Signed version does not match manifest version.');
  return { version, platforms: { 'darwin-aarch64': { signature: sig, url } } };
}
export function run(args) {
  const options = {};
  const allowed = new Set(['version', 'url', 'artifact', 'signature', 'public-key', 'output']);
  for (let i = 0; i < args.length; i += 2) {
    const name = args[i]?.replace(/^--/, '');
    if (!args[i]?.startsWith('--') || !allowed.has(name) || options[name] !== undefined || !args[i + 1] || args[i + 1].startsWith('--')) throw new Error('Use --version --url --artifact --signature --public-key --output exactly once.');
    options[name] = args[i + 1];
  }
  if ([...allowed].some(name => !options[name])) throw new Error('Missing required manifest option.');
  const artifactPath = resolve(options.artifact);
  const outputPath = resolve(options.output);
  if ([artifactPath, resolve(options.signature), resolve(options['public-key'])].includes(outputPath)) throw new Error('Output must not overwrite input files.');
  const size = statSync(artifactPath).size;
  if (size < 2 || size > MAX_BYTES) throw new Error('Artifact exceeds updater size limits.');
  for (const path of [options.signature, options['public-key']]) if (statSync(path).size > 8192) throw new Error('Verification file exceeds size limits.');
  const artifact = readFileSync(artifactPath);
  const manifest = createManifest({ version: options.version, url: options.url, artifact, signature: readFileSync(options.signature, 'utf8'), publicKey: readFileSync(options['public-key'], 'utf8') });
  writeFileSync(outputPath, JSON.stringify(manifest, null, 2) + '\n', { flag: 'wx' });
  return { output: outputPath, bytes: artifact.length, sha256: createHash('sha256').update(artifact).digest('hex') };
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { console.log(JSON.stringify(run(process.argv.slice(2)))); }
  catch (error) { console.error(error.message); process.exitCode = 1; }
}
