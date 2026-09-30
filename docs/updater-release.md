# Bench updater release guide

Bench’s source and update artifacts use the same repository, `jdavies69/bench`. The intended static feed is `https://github.com/jdavies69/bench/releases/latest/download/latest.json`. Development release 0.0.2 and this feed were published September 30; the current Mac upgraded from a feed-enabled 0.0.1 seed through native Check and Restart. The manifest tooling itself does not publish releases. A built-in feed becomes usable only after an approved signed release has been published and a downloaded update has passed acceptance. Apple Developer ID signing and notarization remain separate from updater signing.

## Signing key custody

Keep the existing updater private key outside the checkout. The matching public key is safe to embed in Bench and use for local verification. Do not generate a replacement release key casually: previously installed clients trust the embedded key and cannot accept signatures from an unrelated key. Tauri’s [updater documentation](https://v2.tauri.app/plugin/updater/#signing-updates) explains key generation, artifact signing and the consequences of losing the private key.

Before the first public release, back up the existing private key into an encrypted offline store or approved password manager, together with its public key, any password, and the key’s purpose. Keep an independently stored second recovery copy. Restrict backup access to release maintainers. Test recovery in a temporary directory outside the repository by signing a disposable file and verifying it with the known public key; never publish a test artifact signed by the production key. Record recovery success and the public-key fingerprint, without key contents or passwords, then remove the temporary copy. No automatic backup or key-custody verification is performed by this script.

Never commit a private key, place it in a `.env` file, print it in logs, or upload it as a release asset. Supply its path through the authorized build environment. An unencrypted local key needs encrypted backup and protected local access too. If the key is compromised, stop publication and establish a reviewed recovery/rotation plan; changing the configured public key alone does not repair existing clients.

## Build and inspect the exact artifact

Use a clean verified commit, aligned release version and the reviewed release configuration. Current signing setup can be supplied as a private-key **path**, not key contents:

```sh
TAURI_SIGNING_PRIVATE_KEY=/Users/jfd/.config/bench-release/updater.key \
TAURI_SIGNING_PRIVATE_KEY_PASSWORD='' \
npm run tauri build -- --config src-tauri/tauri.release.conf.json
```

This example reflects the current local key location; do not copy a secret value into the command. If a protected key is used, inject its password through the approved secret environment instead. Tauri produces `Bench.app.tar.gz` and its `.sig` when `createUpdaterArtifacts` is enabled. The updater archive contains the app; a DMG is for first installation and is not the updater payload. Inspect the archive’s members and extracted app, architecture, version and macOS signing/notarization evidence before release. Updater-signature validity does not prove archive structure, code signing, platform support or product behavior.

The signature must bind the release version. Tauri build does this with the current CLI; manual signing must pass `--app-version VERSION`, as shown by `npm run tauri -- signer sign --help`. Keep the exact build artifact unchanged after signing.

## Construct latest.json locally

For a selected stable version, use the public key corresponding to Bench’s embedded updater key:

```sh
node scripts/updater-manifest.mjs \
  --version 0.0.1 \
  --url https://github.com/jdavies69/bench/releases/download/v0.0.1/Bench.app.tar.gz \
  --artifact src-tauri/target/release/bundle/macos/Bench.app.tar.gz \
  --signature src-tauri/target/release/bundle/macos/Bench.app.tar.gz.sig \
  --public-key /Users/jfd/.config/bench-release/updater.key.pub \
  --output /private/tmp/bench-latest.json
```

Replace the version and URL with the approved final values. The script does not select a version or authorize its release. It requires a stable numeric semver, an HTTPS `.app.tar.gz` URL with no credentials/query/fragment, a nonempty gzip payload no larger than Bench’s 128 MiB download limit, a valid Minisign artifact signature and authenticated version comment. Both the artifact signature and trusted-comment signature are checked using Node’s Ed25519 implementation; Tauri’s prehashed BLAKE2b format is supported. It reads local files only, rejects duplicate/unknown arguments, refuses to overwrite inputs or an existing output, and emits a deterministic static manifest containing only `version` and `platforms.darwin-aarch64.{signature,url}`. It prints the exact archive’s SHA-256 and byte count for review. It neither reads private keys nor contacts GitHub.

The [official static-feed schema](https://v2.tauri.app/plugin/updater/#static-json-file) defines the manifest platform keys and signature fields. Rename the reviewed manifest to `latest.json` when preparing release assets. Keep optional notes/date outside the manifest generator so identical inputs always produce identical JSON. Tests include a genuine disposable Tauri CLI signature; its private key was deleted, and its tiny gzip fixture is not an installable app.

## Publication and acceptance

Prepare a reviewable GitHub release in `jdavies69/bench` with the exact versioned updater archive, matching `.sig`, `latest.json`, first-install DMG, release notes, and checksums. For a polished release, macOS release verification and product acceptance must pass. The user separately authorized a development release without Apple signing/notarization; label that exception prominently and verify its updater signatures and actual upgrade. GitHub latest must include this numeric development release for the configured latest endpoint; do not mark it a GitHub prerelease, which that endpoint excludes. Publishing a newer GitHub release without `latest.json` breaks the `/releases/latest/download/latest.json` channel; every release participating in this channel must carry a complete reviewed feed. The archive URL inside the feed stays tied to the specific versioned release.

After authorized publication, download the public feed and archive, compare them with the reviewed local files, and test from an older installed build. Confirm signature verification, ready-to-restart status, explicit restart, new version, saved conversations/artifact histories, and normal access to existing OpenRouter keys. Verify cancellation/errors preserve the installed app and saved work. This download and install acceptance is required before claiming automatic updates work. Bench downloads in the background when configured/enabled and waits for an explicit restart. The live development channel was verified with manual Check through the shared download path; observing a scheduled background check against a future release remains pending.
