# Public release gates

The selected product is Apple Silicon first, with a managed Bench paid plan and included usage. Current BYOK Chat/Website builds remain development artifacts until managed access works. This document describes pending release work; no signing, notarization, credentials, managed service deployment, purchase, or public distribution is completed by the tooling.

## Pending decisions and access

| Gate | Current status | Completion evidence |
| --- | --- | --- |
| CPU architecture | Apple Silicon first selected | `arm64` artifact; no Intel compatibility claim |
| Release version | Still development version `0.0.1` | Agreed version aligned in package.json, Cargo.toml/lock, and tauri.conf.json |
| Minimum macOS | Unselected; Tauri default is not tested support | Compatibility test on selected oldest version, explicit bundle.macOS.minimumSystemVersion |
| Developer ID | No valid local signing identity observed | Authorized identity/private key available; exact team recorded |
| Notarization | No submission performed | Accepted submission, stapled exact artifact, Gatekeeper assessment |
| Managed plan | Selected, implementation/operations pending | Working account access, paid-plan provisioning, server-owned provider credentials, enforced usage cap and failure recovery |
| Pricing/allowance | Not finalized by packaging work | Approved price, included allowance, accounting unit, cap and refund/renewal behavior |
| Native acceptance | Development reviews only | Final downloaded artifact installed and exercised on clean Apple Silicon Mac |
| Distribution | No public release | Approved final artifact, checksums, release notes, installation/support instructions |

Keep release credentials outside source. Do not change credential ACLs to suppress prompts. The Keychain status query avoids reading passwords on startup; actual model calls still require normal secret access. A signed upgrade must preserve local data and verify access to previously saved keys.

## Reproducible development packaging

The manual **Development packaging (unsigned)** GitHub workflow validates frontend/Rust, builds Apple Silicon app/DMG artifacts, and uploads a development-only artifact containing source commit, tool versions, status, and SHA-256 checksums. It has read-only repository permissions, uses ad-hoc signing, supplies no signing/notarization secrets, and never creates a GitHub release. It must be run and observed successfully before claiming CI packaging works.

For a local development build, run the repository validation commands, then:

```sh
npm run tauri build -- --target aarch64-apple-darwin --bundles app,dmg
```

Tauri may sign/notarize when credentials are configured in the launching environment. Use a development environment without Apple notarization credentials when preparing unsigned artifacts. No script in this repository submits notarization or publishes a release.

## Release preparation

Run from a clean committed checkout after the version and minimum macOS have been selected:

```sh
node scripts/release-preflight.mjs --architecture arm64
```

The preflight checks source versions, Git cleanliness, explicit minimum macOS, hardened-runtime configuration, and a valid Developer ID Application identity. It is read-only and does not inspect private-key contents. It intentionally fails on current missing decisions/access. Passing only authorizes a claim of preparation checks passing; it is not product acceptance.

Follow the [official Tauri signing/notarization guide](https://v2.tauri.app/distribute/sign/macos/) with authorized credentials. After a signed/notarized build, verify its exact app bundle using the agreed values:

```sh
node scripts/check-macos-beta.mjs /path/to/Bench.app --architecture arm64 --version AGREED_VERSION --minimum-macos TESTED_VERSION --team-id APPROVED_TEAM
```

Replace the uppercase placeholders before running. Do not modify the checked artifact afterward. Verify the final distributed container and its downloaded app, retain checksums and notarization acceptance, and complete the [clean-machine checklist](macos-beta.md#clean-machine-acceptance). Retain evidence separately from credentials and personal user data.

For the managed plan, clean-machine acceptance must additionally show a new user can obtain managed access without a provider API key, see the plan/allowance accurately, and reach a server-enforced cap without unexpected charges. Test failed payment/provisioning, expired/revoked access, reconnect, concurrent requests, and provider failures. Claims about live billing, usage enforcement, or paid model behavior require observed authorized tests. Packaging success cannot stand in for those checks.

Final publication remains a separate concrete action after the finished artifact, release notes, pricing/usage terms, and verification evidence are reviewable. Keep unsupported outputs and unverified capabilities out of public claims.
