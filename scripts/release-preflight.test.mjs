import test from 'node:test';
import assert from 'node:assert/strict';
import { reviewReleaseSource } from './release-preflight.mjs';

const ready = {
  packageVersion: '0.1.0', cargoVersion: '0.1.0',
  tauri: { version: '0.1.0', bundle: { macOS: { minimumSystemVersion: '12.0' } } },
  dirty: '', architecture: 'arm64',
  identities: { status: 0, output: '  1) ABC123 "Developer ID Application: Example (TEAM123)"\n  1 valid identities found' },
};

test('aligned clean configured source with valid signing identity passes preparation checks', () => {
  assert.equal(reviewReleaseSource(ready).every((check) => check.passed), true);
});

test('development defaults and missing identity remain concrete release blockers', () => {
  const checks = reviewReleaseSource({ ...ready, tauri: { version: '0.1.0', bundle: {} }, identities: { status: 0, output: '0 valid identities found' } });
  assert.deepEqual(checks.filter((check) => !check.passed).map((check) => check.name), ['Explicit minimum macOS', 'Developer ID signing available']);
});

test('wrong signing class or failed command never proves Developer ID availability', () => {
  for (const identities of [{ status: 0, output: '  1) ABC123 "Apple Development: Example (TEAM123)"' }, { status: 1, output: ready.identities.output }]) {
    assert.equal(reviewReleaseSource({ ...ready, identities }).find((check) => check.name === 'Developer ID signing available').passed, false);
  }
});

test('version drift, untracked changes, unknown architecture, and disabled runtime fail', () => {
  const checks = reviewReleaseSource({ ...ready, cargoVersion: '0.0.1', dirty: '?? file', architecture: 'all', tauri: { ...ready.tauri, bundle: { macOS: { minimumSystemVersion: '12.0', hardenedRuntime: false } } } });
  assert.deepEqual(checks.filter((check) => !check.passed).map((check) => check.name), ['Version consistency', 'Clean source', 'Architecture decision', 'Hardened runtime enabled']);
});
