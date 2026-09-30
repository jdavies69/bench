# Roadmap

## Now

- Add signed automatic updates so users do not repeatedly drag/Replace app bundles. Public feed/signing and clean-machine update verification remain release gates.
- Design optional monthly AI-budget-aware model routing and opt-in real stronger-model comparisons; no paid examples without approval and no Bench account/billing service.

- Prepare the Apple Silicon BYOK release with Connect OpenRouter browser sign-in. Accounts/billing remain deferred. Complete OAuth native verification and signing/notarization/clean-install gates; see `docs/public-release.md`.
- Activate the existing output types with real workflows, as requested September 30. All existing types now have bounded workflows; continue authorized live verification and improve concrete failures.
- Live-verify OpenRouter's hosted web-search stream and citations after a specific paid-call authorization.
- Review the Website output for usability and factual accuracy across more specifically authorized live revisions. The existing saved version and section navigation have been reviewed without paid calls.

## Current handoff

- Development download and updater rollout underway: user explicitly authorized web distribution without Apple Developer ID/notarization. Configured GitHub latest feed and installed an enabled 0.0.1 seed locally. Version 0.0.2 aligned across source; installer moved to a blocking worker to keep macOS administrator approval responsive. Full checks pass (130 Rust, 87 frontend, 16 release-tool). Publishing and actual native upgrade acceptance still pending; no paid model calls made.

- Chat response refinement installed September 30: concise default system instructions require one direct answer without repeated conclusions, speculative attachment origins, or unsolicited follow-ups. Hosted search is offered only when the raw user request matches current-information or explicit web-search intent; simple attachment questions use ordinary Chat. 130 Rust tests, Clippy, formatting and signed native build pass; installed bundle matches and opens normally. No paid request was made to verify the new model behavior.

- File attachments are installed and native-verified September 30: composer + opens the Mac chooser, selected filename chips are visibly positioned below the composer, and removal works. User confirmed successful upload. `/Applications/Bench.app` recursively matches the final rebuilt package. Selecting stays local; no paid provider request was made in this attachment cycle.
- Supported uploads: PNG/JPEG/WebP, UTF-8 TXT/MD/CSV/JSON, DOCX, XLSX, and bounded classic-table/Flate text PDFs. Up to five files/20 MiB; images 5 MiB each, document source 10 MiB, extracted text 256 KiB per file/1 MiB per message. XLS, encrypted/unsupported PDFs and scans needing OCR have explicit limitations; see README. Rust staging survives failed DB writes, message+attachments commit atomically, and persisted text/images enter every output’s provider context. Image output uses image uploads as actual references. File contents do not change native search intent or action grants. Updates refuse restart with staged files.
- Attachment validation: 129 Rust tests, 87 frontend tests, 16 release-tool tests, formatting, Clippy, frontend build and signed native package passed. Native verification caught off-screen attachment chips; relative composer positioning was fixed, rebuilt and visibly rechecked. User subsequently sent the fixture and confirmed live Chat read its actual contents; that response exposed excessive repetition and unnecessary hosted searches, addressed by the next Chat refinement.

- Installed Agent/media diagnostics milestone: `/Applications/Bench.app` recursively matches the freshly built signed updater package. 117 Rust tests, 81 frontend tests, 16 release-tool tests, formatting, Clippy and production build pass. Agent is a bounded local conversation-writing workflow, not browsing/host automation; paid Agent generation remains unverified.
- User specifically authorized ONE retry of saved Image request “A beach day in nyc.” Native UI retry returned HTTP 402; mapped message confirms OpenRouter could not fund the request and directs the user to credits/key limit. No image was saved, no second retry was made, and the saved request remains ready to continue. Previous generic message hid this cause; safe specific media errors now remain visible without exposing upstream bodies.
- Optional monthly budget routing is documented in `docs/ai-budget-design.md`; no budget feature or paid comparisons are implemented. Automatic updater code/signatures/tests and release tooling are complete, but public release feed and actual in-place update verification remain pending Apple signing/notarization release gates.

- Repository is now public at https://github.com/jdavies69/bench under MIT, explicitly authorized September 30. Gitleaks and an independent all-history audit found no credential/user-data blockers. History contains ordinary author metadata and local development paths.

- Current UI refinement: Connections and Advanced model settings expose only OpenRouter; removed the other-provider onboarding route. Legacy provider implementation and credentials are preserved. Frontend tests/build pass; rebuilt package installed and native Settings visibly shows only OpenRouter.

- Installed September 30 OAuth/usage update: `/Applications/Bench.app` recursively matches the freshly built package. Connect OpenRouter now opens browser sign-in with PKCE and stores the result directly in Keychain. The user completed authorization and confirmed it works; Computer Use then verified automatic return to Settings with OpenRouter Connected. No paid inference/search was run. Manual key entry remains a Settings fallback.
- Native usage was live-verified against the browser-connected key: Today, Month, Lifetime, and BYOK Month showed $0.00, with Unlimited key limit and Not applicable remaining. These are provider-returned values for the newly created key, not fabricated values or an account balance. Refresh is manual; neither launch nor Settings entry reads the protected credential or requests usage. The earlier check of the old key waited for Keychain authorization; the user allowed it. Computer Use cannot interact with Apple's SecurityAgent.
- Strong PKCE verifier and random callback path, strict bounded loopback HTTP, no redirects, cancellation/timeout, one active attempt, and serialized credential changes keep secrets in Rust/Keychain. Automated tests verify cancellation, stale completion, unmount cancellation, exact callback, exchange, and safe failures. Successful setup never starts a saved prompt automatically.
- Validation: 77 Rust tests, formatting, Clippy, frontend build, 45 frontend tests (39 shipping tests plus 6 unintegrated output-viewer tests), and 11 release-tool tests passed. Package rebuilt after the unmount-cancellation fix. Existing projects, conversations, preferences, and manual provider connections remain available. Public distribution still requires stable Developer ID signing, notarization, minimum macOS selection, and clean-machine verification.
- Latest release choice remains Apple Silicon BYOK, no Bench accounts/billing. Supabase/Stripe and a monthly fixed balance with roughly 5% cost markup are future ideas only. No Bench domain, service accounts, or Apple Developer membership exists. Managed accounting is parked at `future/managed-usage-prototype` (`46feacb`), with 33 passing local tests; no service is deployed or wired into desktop, and the temporary PostgreSQL cluster is stopped.
- Output integration now registers Document, Presentation, Image, Voice, and bounded declarative Application workflows. Immutable revisions, recovery, exact native approvals, safe exports, media playback, and per-revision Application input persistence are implemented. 113 Rust tests, Clippy, formatting, 65 frontend tests, 16 release-tool tests, and frontend build pass. No paid generation was run; signed updater package built and installed at `/Applications/Bench.app` with recursive bundle match. Native Settings verifies the honest unconfigured updater state; native picker includes the newly registered outputs. Actual paid generation remains unverified. Agent now has a native bounded local writing/synthesis tool loop with staged reports, exact authorization, immutable history/export, four-round/response limits and a 300-second overall deadline; live paid behavior remains unverified. The OpenRouter SDK supports tool loops but needs a runtime decision and Rust-enforced tool authorization; Ori Harness is not an embedded native output runtime.
- Signed updater code and Settings controls are integrated with real signature/version-bound fixtures, bounded transport and failure tests. Release signing key remains outside Git. Public feed is unconfigured; user authorized making this source repository public and delegated license choice (MIT). Public release artifacts still require macOS release gates and a real in-place update test. Monthly budget routing remains a design proposal, with opt-in real paid comparisons and no Bench account/billing.
- Compact borderless output picker, saved Website version 3 and section navigation, and blank/active conversations with both sidebar states were previously reviewed natively. The previous build marked unfinished outputs Coming later; newly registered workflows must pass packaged/native checks before launch claims.
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
