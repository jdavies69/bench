import test from 'node:test';
import assert from 'node:assert/strict';
import { inspectBundle } from './check-macos-beta.mjs';

function fixture(overrides = {}) {
  const calls = [];
  const run = (command, args) => {
    calls.push([command, args]);
    const key = command === '/usr/libexec/PlistBuddy' ? args[1] : `${command} ${args[0]}`;
    const values = {
      'Print :CFBundleIdentifier': 'app.bench.desktop',
      'Print :CFBundleExecutable': 'bench',
      'Print :CFBundleShortVersionString': '0.0.1',
      'Print :LSMinimumSystemVersion': '11.0',
      'codesign --verify': '',
      'codesign --display': 'Authority=Developer ID Application: Example (TEAM123)\nAuthority=Developer ID Certification Authority\nTeamIdentifier=TEAM123\nCodeDirectory v=20500 flags=0x10000(runtime)\n',
      'lipo -archs': 'x86_64 arm64',
      'spctl --assess': 'accepted\nsource=Notarized Developer ID\n',
      'xcrun stapler': 'The validate action worked!',
    };
    return overrides[key] ?? { status: 0, output: values[key] ?? '' };
  };
  return { calls, options: { run, exists: () => true } };
}

test('complete signed/notarized bundle passes without mutating or launching it', () => {
  const { calls, options } = fixture();
  assert.equal(inspectBundle('/tmp/Beta with spaces.app', options).every((check) => check.passed), true);
  assert.equal(calls.length, 7);
  assert.equal(calls.find(([command]) => command === 'codesign')[1].at(-1), '/tmp/Beta with spaces.app');
  assert.equal(calls.some(([, args]) => args.includes('--sign') || args.includes('staple')), false);
});

test('local ad-hoc build fails even when integrity and architecture pass', () => {
  const { options } = fixture({ 'codesign --display': { status: 0, output: 'Signature=adhoc\nTeamIdentifier=not set\nCodeDirectory flags=0x20002(adhoc,linker-signed)' } });
  const checks = inspectBundle('/tmp/Bench.app', options);
  assert.equal(checks.find((check) => check.name === 'Signature integrity').passed, true);
  assert.equal(checks.find((check) => check.name === 'Developer ID identity').passed, false);
  assert.equal(checks.find((check) => check.name === 'Hardened runtime').passed, false);
});

test('Gatekeeper acceptance without notarization and missing offline ticket fail independently', () => {
  const { options } = fixture({
    'spctl --assess': { status: 0, output: 'accepted\nsource=Developer ID\n' },
    'xcrun stapler': { status: 65, output: 'No stapled ticket' },
  });
  assert.deepEqual(inspectBundle('/tmp/Bench.app', options).filter((check) => !check.passed).map((check) => check.name), ['Gatekeeper notarization', 'Stapled notarization ticket']);
});

test('command failures do not masquerade as successful evidence', () => {
  const { options } = fixture({ 'codesign --verify': { status: null, output: '', error: 'spawn codesign ENOENT' }, 'lipo -archs': { status: 1, output: 'arm64' } });
  const checks = inspectBundle('/tmp/Bench.app', options);
  assert.equal(checks.find((check) => check.name === 'Signature integrity').passed, false);
  assert.equal(checks.find((check) => check.name === 'Release architecture').passed, false);
});

test('release expectations reject wrong version, minimum OS, team, and incomplete universal binaries', () => {
  const { options } = fixture({ 'lipo -archs': { status: 0, output: 'arm64' } });
  const checks = inspectBundle('/tmp/Bench.app', { ...options, architecture: 'universal', version: '0.1.0', minimumMacOS: '12.0', teamId: 'OTHERTEAM' });
  assert.deepEqual(checks.filter((check) => !check.passed).map((check) => check.name), ['Release version', 'Minimum macOS', 'Expected signing team', 'Release architecture']);
});

test('Intel-only artifacts can be checked explicitly without claiming universal support', () => {
  const { options } = fixture({ 'lipo -archs': { status: 0, output: 'x86_64' } });
  assert.equal(inspectBundle('/tmp/Bench.app', { ...options, architecture: 'x86_64', version: '0.0.1', minimumMacOS: '11.0', teamId: 'TEAM123' }).every((check) => check.passed), true);
});

test('missing bundle and unsafe executable stop before assessment', () => {
  const missing = fixture();
  assert.equal(inspectBundle('/tmp/Bench.app', { ...missing.options, exists: () => false })[0].passed, false);
  assert.equal(missing.calls.length, 0);
  const invalid = fixture({ 'Print :CFBundleExecutable': { status: 0, output: '../other' } });
  assert.equal(inspectBundle('/tmp/Bench.app', invalid.options).at(-1).passed, false);
  assert.equal(invalid.calls.length, 2);
});
