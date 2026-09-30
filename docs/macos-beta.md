# External Mac beta

## Current boundary

Bench's local builds are development artifacts. The September 30 installed app was Apple Silicon (`arm64`), ad-hoc signed, with no Developer ID team. External distribution is pending signing, notarization, and a clean-machine installation. CI validates source; it does not currently produce a distributable beta.

The installed bundle declares macOS 10.13 through Tauri's default. That is metadata, not a tested compatibility claim. Select and configure an actual minimum macOS version after testing. Intel support is also unverified; label the initial artifact Apple Silicon unless an Intel/universal build is tested.

## Artifact checks

Run on macOS with Node and Xcode command-line tools:

```sh
node scripts/check-macos-beta.mjs /Applications/Bench.app
node --test scripts/check-macos-beta.test.mjs
```

The verifier reads the supplied bundle and checks identity, executable, signature integrity, Developer ID authority/team, hardened runtime, Apple Silicon architecture, Gatekeeper notarization, and the stapled ticket. It performs no signing, stapling, launch, or bundle modification. Exit 0 means these artifact checks passed; exit 1 means a check failed; exit 2 means incorrect invocation. It requires Apple Silicon support and is not an Intel-only release validator. A pass does not establish clean-machine behavior, compatibility, Website factual quality, or live search behavior.

## Signing and notarization preparation

Before a release build, choose the Developer ID owner, beta architecture, and tested minimum macOS version. Obtain an authorized Developer ID Application certificate/private key and notarization credentials. Do not commit credentials or certificate exports.

Use the current [Tauri macOS signing guide](https://v2.tauri.app/distribute/sign/macos/) and [Apple notarization guide](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution) for the credential setup. Tauri accepts the installed signing identity through `APPLE_SIGNING_IDENTITY` or `bundle.macOS.signingIdentity`; hardened runtime should remain enabled. Follow the selected notarization credential path in the official guide rather than placing secrets in a repository config.

Once credentials are authorized and available, build app/DMG artifacts with `npm run tauri build -- --bundles app,dmg`. Verify the exact signed app with the checker above. Validate its stapled ticket and assess the distributed container as applicable. Retain notarization acceptance, artifact SHA-256, source commit, version, architecture, and tested macOS version in release evidence. Do not rebuild or modify accepted artifacts before sharing them; changed artifacts require fresh verification. Credentials, signing, notarization submission, and distribution have not been performed by this preparation work.

## Clean-machine acceptance

Use a separate Mac or clean test account with no existing Bench database, Keychain keys, or provider environment variables. Download the actual beta through a browser so the real quarantine/Gatekeeper path is exercised; copying a development bundle locally is insufficient.

1. Install from the final container into Applications and launch normally. Record any Gatekeeper and Keychain prompts. Verify the build/version and architecture against release evidence.
2. Inspect blank Chat, active conversation, expanded/collapsed sidebar, and Settings. Confirm a readable window at the minimum supported size and normal Mac keyboard navigation.
3. Discover Settings → Connections from a fresh workspace. Confirm local prompts survive a missing-key error. With a specifically authorized test key, connect a provider and run an authorized Chat request; reconnect after restart without re-entering the key. Do not print or record the key in test evidence.
4. Verify conversations, projects, sidebar preference, manual moves, and settings survive restart. Check failed/offline requests preserve the user's text and offer recovery.
5. With specific paid-call authorization, create and revise a static Website; check factual placeholders, preview isolation, version recovery, and restart persistence. Existing automated fake-provider tests do not replace this acceptance check.
6. Live OpenRouter search still requires separate specific authorization. Record citations and failure handling only after an authorized run.
7. Replace the app with the next signed build and verify existing local data and Keychain access survive. Test Keychain access across the development-to-Developer-ID signing transition before recommending upgrades to existing users.

Record each check as passed, failed, or pending with the exact artifact and environment. Share only after the artifact checks and required manual acceptance pass. No managed accounts, billing, automatic updater, deployment integration, or additional outputs are part of this beta preparation.
