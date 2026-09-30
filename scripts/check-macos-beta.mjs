#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const execute = (command, args) => {
  const result = spawnSync(command, args, { encoding: 'utf8', timeout: 30_000 });
  return { status: result.status, output: `${result.stdout ?? ''}${result.stderr ?? ''}`, error: result.error?.message };
};

// Checks the exact supplied bundle. This never signs, staples, launches, or alters it.
export function inspectBundle(bundle, { run = execute, exists = existsSync, architecture = 'arm64', version, minimumMacOS, teamId } = {}) {
  const checks = [];
  const check = (name, passed, detail) => checks.push({ name, passed, detail });
  const plist = `${bundle}/Contents/Info.plist`;
  if (!exists(plist)) return [{ name: 'Bundle', passed: false, detail: `Missing ${plist}` }];
  const property = (key) => run('/usr/libexec/PlistBuddy', ['-c', `Print :${key}`, plist]);
  const identifier = property('CFBundleIdentifier');
  check('Bundle identity', identifier.status === 0 && identifier.output.trim() === 'app.bench.desktop', identifier.output.trim());
  for (const [expected, key, label] of [[version, 'CFBundleShortVersionString', 'Release version'], [minimumMacOS, 'LSMinimumSystemVersion', 'Minimum macOS']]) {
    if (expected !== undefined) {
      const actual = property(key);
      check(label, actual.status === 0 && actual.output.trim() === expected, `expected ${expected}, found ${actual.output.trim()}`);
    }
  }
  const executable = property('CFBundleExecutable');
  const name = executable.output.trim();
  if (executable.status !== 0 || !name || name.includes('/') || name === '.' || name === '..') {
    check('Executable', false, 'Missing or invalid CFBundleExecutable');
    return checks;
  }
  const binary = `${bundle}/Contents/MacOS/${name}`;
  check('Executable', exists(binary), binary);
  const signature = run('codesign', ['--verify', '--deep', '--strict', '--verbose=2', bundle]);
  check('Signature integrity', signature.status === 0, signature.error ?? signature.output.trim());
  const details = run('codesign', ['--display', '--verbose=4', bundle]);
  const authorities = details.output.split('\n').filter((line) => line.startsWith('Authority='));
  check('Developer ID identity', details.status === 0 && authorities[0]?.startsWith('Authority=Developer ID Application:') === true && /^TeamIdentifier=(?!not set\s*$)\S+/m.test(details.output) && !/^Signature=adhoc$/m.test(details.output), authorities.join('; ') || 'No Developer ID signing authority');
  if (teamId !== undefined) check('Expected signing team', details.status === 0 && details.output.split('\n').includes(`TeamIdentifier=${teamId}`), `expected ${teamId}`);
  check('Hardened runtime', details.status === 0 && /flags=[^\n]*\bruntime\b/.test(details.output), details.output.split('\n').find((line) => line.startsWith('CodeDirectory')) ?? 'No code directory');
  const architectures = run('lipo', ['-archs', binary]);
  const archs = architectures.output.trim().split(/\s+/);
  const required = architecture === 'universal' ? ['arm64', 'x86_64'] : [architecture];
  check('Release architecture', ['arm64', 'x86_64', 'universal'].includes(architecture) && architectures.status === 0 && required.every((arch) => archs.includes(arch)), `expected ${architecture}, found ${architectures.output.trim()}`);
  const gatekeeper = run('spctl', ['--assess', '--type', 'execute', '--verbose=4', bundle]);
  check('Gatekeeper notarization', gatekeeper.status === 0 && /^source=Notarized Developer ID$/m.test(gatekeeper.output), gatekeeper.error ?? gatekeeper.output.trim());
  const ticket = run('xcrun', ['stapler', 'validate', bundle]);
  check('Stapled notarization ticket', ticket.status === 0, ticket.error ?? ticket.output.trim());
  return checks;
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const args = process.argv.slice(2);
  const validFlags = new Map([['--architecture', 'architecture'], ['--version', 'version'], ['--minimum-macos', 'minimumMacOS'], ['--team-id', 'teamId']]);
  const options = {};
  let valid = args.length >= 1 && !args[0].startsWith('-') && args.length % 2 === 1;
  for (let index = 1; valid && index < args.length; index += 2) {
    const key = validFlags.get(args[index]);
    if (!key || Object.hasOwn(options, key) || !args[index + 1] || args[index + 1].startsWith('-')) valid = false;
    else options[key] = args[index + 1];
  }
  if (options.architecture && !['arm64', 'x86_64', 'universal'].includes(options.architecture)) valid = false;
  if (!valid) {
    console.error('Usage: node scripts/check-macos-beta.mjs /path/to/Bench.app [--architecture arm64|x86_64|universal] [--version VERSION] [--minimum-macos VERSION] [--team-id TEAM]');
    process.exitCode = 2;
  } else {
    const checks = inspectBundle(resolve(args[0]), options);
    for (const { name, passed, detail } of checks) console.log(`${passed ? 'PASS' : 'FAIL'} ${name}${detail ? `: ${detail}` : ''}`);
    const passed = checks.every((check) => check.passed);
    console.log(passed ? 'Artifact checks passed. Clean-machine installation and onboarding still require manual verification.' : 'Artifact checks failed. This bundle is not ready for an external Mac beta.');
    process.exitCode = passed ? 0 : 1;
  }
}
