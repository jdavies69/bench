# Roadmap

## Now

- Prepare the bare-minimum Apple Silicon BYOK public release. Accounts/billing are deferred by the latest user choice; focus on signing, notarization, clean installation, and existing Chat/Website reliability. See `docs/public-release.md`.
- Live-verify OpenRouter's hosted web-search stream and citations after a specific paid-call authorization.
- Review the Website output for usability and factual accuracy across more specifically authorized live revisions. The existing saved version and section navigation have been reviewed without paid calls.

## Current handoff

- The installed app contains code commit `2f29b4b`, rebuilt September 30 with borderless Auto, Settings About/version/data-flow copy, missing-connection recovery, IME Enter handling, and provider truncation rejection. Blank chat, active conversation, both sidebar states, Settings, and About were visually verified; no paid calls were made. A recursive comparison confirmed the package matched the installation before the build copy was moved to `/Users/jfd/.Trash/Bench-build-2f29b4b.app`. `/Applications/Bench.app` is the current installation.
- Latest release choice: Apple Silicon BYOK first, no accounts/billing. Supabase/Stripe and a monthly fixed balance with roughly 5% cost markup are future ideas only; none of the Bench-owned service accounts, domain, or Apple Developer membership exists. Managed accounting is parked at `future/managed-usage-prototype` (`46feacb`), with 33 passing local tests including real PostgreSQL concurrency/restart coverage. No service is deployed or wired into desktop; the temporary test cluster was stopped.
- The approved compact output picker is implemented and visually checked in blank chat and an active conversation. It uses neutral icons, a selected checkmark, and a Coming later section. Native keyboard selection, Escape, and outside-click dismissal were verified without sending requests. The menu opens above when it fits, otherwise below or with scrolling; resize containment has regression coverage. Existing placeholder outputs remain selectable.
- Computer Use worked this session. Blank chat, a saved active conversation, expanded/collapsed sidebar, Settings, and saved Website version 3 were visually reviewed. Website Services navigation stayed at `about:srcdoc#services`, scrolled, and visibly rendered the destination. No live generation/revision was run.
- Fixed repeated startup Keychain authentication: macOS connection status now queries attributes without reading secret data. The updated app launched twice and Settings showed OpenRouter Connected without authentication. Actual model use still reads the protected credential and has not been exercised after this rebuild. Ad-hoc signatures can prompt on first secret use after updates; `security find-identity -v -p codesigning` found no valid signing identities. Stable signing remains a release prerequisite.
- A live OpenRouter web-search test has **not** been authorized or run. The earlier approval to run one live Website test does not cover search. Ask for specific authorization before a paid model/search request, then check citations and failure handling in the installed build.

## Next

- Prepare an external Mac beta: choose signing owner, obtain authorized credentials, select/test the minimum macOS version and architecture, then notarize and verify clean-machine installation and onboarding. See `docs/macos-beta.md` and `scripts/check-macos-beta.mjs`. Current artifact checks correctly fail Developer ID, signature sealing, hardened runtime, Gatekeeper, and stapled-ticket requirements; no external beta was distributed.

## Later

- Expand outputs only after Website is excellent.
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
- 21 frontend, 11 release-tool, and 62 Rust tests, frontend build, formatting, Clippy, and packaged app build passed for code commit `2f29b4b`; its GitHub CI passed. Rust loopback tests require execution outside the restrictive sandbox. External distribution remains unsigned/unnotarized; preflight correctly fails missing minimum macOS and Developer ID identity. The installed artifact additionally fails signature sealing, runtime, Gatekeeper, and ticket checks; stable signing is required before distribution.
