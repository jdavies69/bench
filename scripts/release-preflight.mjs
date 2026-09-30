#!/usr/bin/env node
import { readFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

export function reviewReleaseSource({ packageVersion, tauri, cargoVersion, dirty, identities, architecture }) {
  const checks = [];
  const check = (name, passed, detail) => checks.push({ name, passed, detail });
  check('Version consistency', packageVersion === tauri.version && packageVersion === cargoVersion, `npm=${packageVersion}; Tauri=${tauri.version}; Cargo=${cargoVersion}`);
  check('Clean source', dirty === '', dirty ? 'Uncommitted or untracked files present' : 'No uncommitted changes');
  check('Architecture decision', ['arm64', 'x86_64', 'universal'].includes(architecture), architecture ?? 'Pass --architecture after deciding tested targets');
  const minimum = tauri.bundle?.macOS?.minimumSystemVersion;
  check('Explicit minimum macOS', typeof minimum === 'string' && /^\d+\.\d+(?:\.\d+)?$/.test(minimum), minimum ?? 'Set bundle.macOS.minimumSystemVersion after compatibility testing');
  check('Hardened runtime enabled', tauri.bundle?.macOS?.hardenedRuntime !== false, 'Tauri defaults hardened runtime to true');
  check('Developer ID signing available', identities.status === 0 && /^\s*\d+\) [A-Fa-f0-9]+ "Developer ID Application:[^\n]+"/m.test(identities.output), 'Requires a valid Developer ID Application identity with private key; this check does not access or modify it');
  return checks;
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const args = process.argv.slice(2);
  if (args.length !== 2 || args[0] !== '--architecture' || !['arm64', 'x86_64', 'universal'].includes(args[1])) {
    console.error('Usage (from repository root): node scripts/release-preflight.mjs --architecture arm64|x86_64|universal');
    process.exitCode = 2;
  } else {
    try {
      const pkg = JSON.parse(readFileSync('package.json', 'utf8'));
      const tauri = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8'));
      const cargo = readFileSync('src-tauri/Cargo.toml', 'utf8');
      const cargoVersion = cargo.match(/\[package\]([\s\S]*?)(?=\n\[|$)/)?.[1].match(/^version\s*=\s*"([^"]+)"/m)?.[1];
      const git = spawnSync('git', ['status', '--porcelain', '--untracked-files=all'], { encoding: 'utf8', timeout: 30_000 });
      if (git.status !== 0) throw new Error('Could not inspect Git state');
      const signing = spawnSync('security', ['find-identity', '-v', '-p', 'codesigning'], { encoding: 'utf8', timeout: 30_000 });
      const checks = reviewReleaseSource({ packageVersion: pkg.version, tauri, cargoVersion, dirty: git.stdout.trim(), identities: { status: signing.status, output: `${signing.stdout ?? ''}${signing.stderr ?? ''}` }, architecture: args[1] });
      for (const { name, passed, detail } of checks) console.log(`${passed ? 'PASS' : 'FAIL'} ${name}: ${detail}`);
      console.log('This preflight never builds, signs, submits notarization, or publishes. Passing does not replace artifact and clean-machine checks.');
      process.exitCode = checks.every((check) => check.passed) ? 0 : 1;
    } catch (error) {
      console.error(`Release preflight failed: ${error.message}`);
      process.exitCode = 1;
    }
  }
}
