import test from 'node:test';
import assert from 'node:assert/strict';
import { generateKeyPairSync, createHash, sign } from 'node:crypto';
import { gzipSync } from 'node:zlib';
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createManifest, run } from './updater-manifest.mjs';

function fixture(comment = 'timestamp:1\tfile:Bench.app.tar.gz\tversion:1.2.3') {
  const { publicKey, privateKey } = generateKeyPairSync('ed25519');
  const key = publicKey.export({ type: 'spki', format: 'der' }).subarray(-32);
  const id = Buffer.from('0102030405060708', 'hex');
  const artifact = gzipSync(Buffer.from('fixture bytes; archive structure acceptance is a separate release gate'));
  const raw = sign(null, createHash('blake2b512').update(artifact).digest(), privateKey);
  const global = sign(null, Buffer.concat([raw, Buffer.from(comment)]), privateKey);
  return {
    version: '1.2.3', url: 'https://example.com/v1.2.3/Bench.app.tar.gz', artifact,
    publicKey: Buffer.from(`untrusted comment: disposable public key\n${Buffer.concat([Buffer.from('Ed'), id, key]).toString('base64')}\n`).toString('base64'),
    signature: Buffer.from(`untrusted comment: disposable signature\n${Buffer.concat([Buffer.from('ED'), id, raw]).toString('base64')}\ntrusted comment: ${comment}\n${global.toString('base64')}\n`).toString('base64'),
  };
}
test('manifest is deterministic, Apple Silicon only, and verifies both signatures', () => {
  const f = fixture();
  const expected = { version: '1.2.3', platforms: { 'darwin-aarch64': { signature: f.signature, url: f.url } } };
  assert.deepEqual(createManifest(f), expected);
  assert.deepEqual(createManifest(f), expected);
  const artifact = Buffer.from(f.artifact); artifact[artifact.length-1] ^= 1;
  assert.throws(() => createManifest({ ...f, artifact }), /verification failed/);
  assert.throws(() => createManifest({ ...f, publicKey: fixture().publicKey }), /verification failed/);
  const text = Buffer.from(f.signature, 'base64').toString().replace('version:1.2.3', 'version:2.0.0');
  assert.throws(() => createManifest({ ...f, signature: Buffer.from(text).toString('base64') }), /verification failed/);
});
test('signed version must be present exactly once and agree with stable release', () => {
  for (const comment of ['timestamp:1', 'version:2.0.0', 'version:1.2.3\tversion:1.2.3']) assert.throws(() => createManifest(fixture(comment)), /Signed version/);
  for (const version of ['v1.2.3', '01.2.3', '1.2', '1.2.3-beta.1', '1.2.3+build', '1.2.3\n', '18446744073709551616.0.0']) assert.throws(() => createManifest({ ...fixture(), version }), /semver/);
});
test('invalid URLs, containers and malformed encodings are rejected', () => {
  const f = fixture();
  for (const url of ['http://example.com/Bench.app.tar.gz', 'https://user:pass@example.com/Bench.app.tar.gz', 'https://example.com/Bench.app.tar.gz#secret', 'https://example.com/Bench.app.tar.gz?token=secret', 'https://example.com/Bench.dmg', 'https://example.com/Bench.app.tar.gz\n', 'garbage']) assert.throws(() => createManifest({ ...f, url }));
  for (const artifact of [Buffer.alloc(0), Buffer.from('html'), Buffer.alloc(128*1024*1024+1)]) assert.throws(() => createManifest({ ...f, artifact }), /archive/);
  for (const signature of ['', '!!!!', Buffer.from('not minisign').toString('base64')]) assert.throws(() => createManifest({ ...f, signature }));
});
test('CLI reads only local public material and cannot overwrite inputs or existing output', () => {
  const dir = mkdtempSync(join(tmpdir(), 'bench-manifest-test-'));
  try {
    const f = fixture(); const artifact = join(dir, 'Bench.app.tar.gz'); const signature = join(dir, 'update.sig'); const key = join(dir, 'update.pub'); const output = join(dir, 'latest.json');
    writeFileSync(artifact, f.artifact); writeFileSync(signature, f.signature); writeFileSync(key, f.publicKey);
    const args = ['--version', f.version, '--url', f.url, '--artifact', artifact, '--signature', signature, '--public-key', key, '--output', output];
    const evidence = run(args);
    assert.equal(evidence.sha256, createHash('sha256').update(f.artifact).digest('hex'));
    assert.deepEqual(JSON.parse(readFileSync(output)), createManifest(f));
    assert.throws(() => run(args), /EEXIST/);
    assert.throws(() => run([...args.slice(0,-1), artifact]), /overwrite/);
    assert.throws(() => run([...args, '--version', '2.0.0']));
    assert.throws(() => run(['--secret-key', 'never-read']));
  } finally { rmSync(dir, { recursive: true }); }
});

test('accepts real Tauri CLI prehashed signed-version fixture', () => {
  const input = name => readFileSync(new URL(`./fixtures/${name}`, import.meta.url));
  const manifest = createManifest({ version: '1.2.3', url: 'https://example.com/Bench.app.tar.gz', artifact: input('updater-test.app.tar.gz'), signature: input('updater-test.app.tar.gz.sig').toString(), publicKey: input('updater-test.pub').toString() });
  assert.equal(manifest.version, '1.2.3');
});
