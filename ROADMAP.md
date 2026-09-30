# Roadmap

## Now

- Live-verify OpenRouter's hosted web-search stream and citations after a specific paid-call authorization.
- Review the Website output for usability and factual accuracy across more specifically authorized live revisions. The existing saved version and section navigation have been reviewed without paid calls.

## Current handoff

- The installed app contains code commit `4acea12`, packaged and installed on September 30 at `/Applications/Bench.app`; a recursive comparison confirms it matches `src-tauri/target/release/bundle/macos/Bench.app`. Subsequent beta-checker and documentation commits do not change the app code. The earlier installation remains backed up at `/private/tmp/Bench-prior-20260930.app`.
- Computer Use worked this session. Blank chat, a saved active conversation, expanded/collapsed sidebar, Settings, and saved Website version 3 were visually reviewed. Website Services navigation stayed at `about:srcdoc#services`, scrolled, and visibly rendered the destination. No live generation/revision was run.
- Fixed repeated startup Keychain authentication: macOS connection status now queries attributes without reading secret data. The updated app launched twice and Settings showed OpenRouter Connected without authentication. Actual model use still reads the protected credential and has not been exercised after this rebuild. Ad-hoc signatures can prompt on first secret use after updates; `security find-identity -v -p codesigning` found no valid signing identities. Stable signing remains a release prerequisite.
- A live OpenRouter web-search test has **not** been authorized or run. The earlier approval to run one live Website test does not cover search. Ask for specific authorization before a paid model/search request, then check citations and failure handling in the installed build.

## Next

- Prepare an external Mac beta: choose signing owner, obtain authorized credentials, select/test the minimum macOS version and architecture, then notarize and verify clean-machine installation and onboarding. See `docs/macos-beta.md` and `scripts/check-macos-beta.mjs`. Current artifact checks correctly fail Developer ID, signature sealing, hardened runtime, Gatekeeper, and stapled-ticket requirements; no external beta was distributed.

## Later

- Expand outputs only after Website is excellent.
- Add service connections when a concrete output flow needs them.
- Optional accounts and monthly managed usage are designed in `docs/managed-usage-milestone.md`; implementation is deferred.

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
- 13 frontend, 5 beta-checker, and 60 Rust tests, frontend build, formatting, Clippy, and packaged app build passed for this milestone. GitHub CI status should be checked for the pushed HEAD. External distribution remains unsigned/unnotarized.
