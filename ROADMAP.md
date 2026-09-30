# Roadmap

## Now

- Prepare the Apple Silicon BYOK release with Connect OpenRouter browser sign-in. Accounts/billing remain deferred. Complete OAuth native verification and signing/notarization/clean-install gates; see `docs/public-release.md`.
- Activate the existing output types with real workflows, as requested September 30. Document/Presentation and media work is in progress; evaluate OpenRouter Agent SDK reuse for Agent/Application before adding a custom runtime.
- Live-verify OpenRouter's hosted web-search stream and citations after a specific paid-call authorization.
- Review the Website output for usability and factual accuracy across more specifically authorized live revisions. The existing saved version and section navigation have been reviewed without paid calls.

## Current handoff

- Installed September 30 OAuth/usage update: `/Applications/Bench.app` recursively matches the freshly built package. Connect OpenRouter now opens browser sign-in with PKCE and stores the result directly in Keychain. The user completed authorization and confirmed it works; Computer Use then verified automatic return to Settings with OpenRouter Connected. No paid inference/search was run. Manual key entry remains a Settings fallback.
- Native usage was live-verified against the browser-connected key: Today, Month, Lifetime, and BYOK Month showed $0.00, with Unlimited key limit and Not applicable remaining. These are provider-returned values for the newly created key, not fabricated values or an account balance. Refresh is manual; neither launch nor Settings entry reads the protected credential or requests usage. The earlier check of the old key waited for Keychain authorization; the user allowed it. Computer Use cannot interact with Apple's SecurityAgent.
- Strong PKCE verifier and random callback path, strict bounded loopback HTTP, no redirects, cancellation/timeout, one active attempt, and serialized credential changes keep secrets in Rust/Keychain. Automated tests verify cancellation, stale completion, unmount cancellation, exact callback, exchange, and safe failures. Successful setup never starts a saved prompt automatically.
- Validation: 77 Rust tests, formatting, Clippy, frontend build, 45 frontend tests (39 shipping tests plus 6 unintegrated output-viewer tests), and 11 release-tool tests passed. Package rebuilt after the unmount-cancellation fix. Existing projects, conversations, preferences, and manual provider connections remain available. Public distribution still requires stable Developer ID signing, notarization, minimum macOS selection, and clean-machine verification.
- Latest release choice remains Apple Silicon BYOK, no Bench accounts/billing. Supabase/Stripe and a monthly fixed balance with roughly 5% cost markup are future ideas only. No Bench domain, service accounts, or Apple Developer membership exists. Managed accounting is parked at `future/managed-usage-prototype` (`46feacb`), with 33 passing local tests; no service is deployed or wired into desktop, and the temporary PostgreSQL cluster is stopped.
- User explicitly requested all existing outputs become live. Preserve unfinished worktree files: `artifact.rs`, `text_outputs.rs`, `media.rs`, Document/Presentation viewers/tests, the Markdown image-blocking prop, and `docs/output-activation-plan.md`. They are not registered as supported workflows. Artifact/text tests passed during temporary module registration; media has not been compiled/tested. Those Rust modules were removed from `lib.rs` to ship the coherent OAuth/usage milestone. Ori Harness configures existing coding CLIs, not native output workspaces; assess the separate OpenRouter Agent SDK for Agent/Application reuse next.
- Compact borderless output picker, saved Website version 3 and section navigation, and blank/active conversations with both sidebar states were previously reviewed natively. The current installed UI still honestly marks unfinished outputs Coming later.
- Paid OpenRouter search is not authorized. Ask for specific authorization before a live model/search request, then verify citation and failure behavior.

## Next

- Prepare an external Mac beta: choose signing owner, obtain authorized credentials, select/test the minimum macOS version and architecture, then notarize and verify clean-machine installation and onboarding. See `docs/macos-beta.md` and `scripts/check-macos-beta.mjs`. Current artifact checks correctly fail Developer ID, signature sealing, hardened runtime, Gatekeeper, and stapled-ticket requirements; no external beta was distributed.

## Later

- Add further output types only when a concrete need exists; complete the current picker’s outputs first.
- Add service connections when a concrete output flow needs them.
- Accounts/billing, cloud sync, and additional integrations remain deferred. Keep managed accounting prototypes separate from the shipping desktop release.

## Completed

- Native Tauri 2 shell with React/TypeScript and Rust; local SQLite and Keychain credentials.
- Sparse canvas, cursor-reactive dot grid, floating collapsible sidebar, centered composer, quiet Settings.
- Streaming Chat, Markdown, local search, conservative project grouping, manual reassignment, and retry/error handling.
- Provider-independent model and web-search interfaces; four model providers supported.
- Auto intent routing and adaptive output workspace; static Website generation, file patches, sandboxed preview, and one live generation.
- Immutable Website history, rollback, last known-good recovery, bounded reads/generation, and preview security tests.
- A live Laundros Website revision changed only `style.css`, advanced to version 3, and survived app relaunch. An initial timeout preserved the prompt and version 2; a retry succeeded with a longer Website timeout.
- Rust-enforced execution and approval policy with exact, one-use action grants.
- First-class output definitions and modules; no additional output workflow was added.
- Tool activity persists with its saved user message; the UI shows concise activity without raw debug data.
- OpenRouter-hosted web search uses the existing key, with capped tool calls, source checks for current answers, and preserved legacy credentials. Mock tests pass; live behavior awaits verification.
- Realistic project and Auto output intent regressions cover explicit artifacts, informational questions, ambiguous projects, and weak new-folder signals.
- Private GitHub repository and macOS CI workflow created.
- Website previews retain safe same-page section links; explicit `about:srcdoc` binding prevents navigation to Bench's inherited base URL. Output entry motion no longer starts invisible, so paused WebKit animations cannot hide the panel.
- Case-insensitive Website filename collisions are rejected before save; regression tests verify last-good preservation, history, and restart-safe capitalization renames.
- A read-only Mac beta checker and clean-machine acceptance guide are in place; `npm test` includes its fixture tests.
- Compact output picker replaces the stock dropdown; five regressions cover selection, keyboard/type-ahead, dismissal, deferred outputs, and resize containment.
- 30 frontend, 11 release-tool, and 62 Rust tests, frontend build, formatting, Clippy, and packaged app build passed for code commit `c910470`. Previous milestone CI (`90f42f8`) passed; check the newest pushed HEAD separately. Rust loopback tests require execution outside the restrictive sandbox. External distribution remains unsigned/unnotarized; preflight correctly fails missing minimum macOS and Developer ID identity. Artifact signature/runtime/Gatekeeper/ticket checks remain release gates; stable signing is required before distribution.
