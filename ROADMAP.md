# Roadmap

## Now

- Review active conversation, sidebar, Settings, and design-token consistency against the running app.
- Review the Website output for usability and factual accuracy across more live revisions.

## Next

- Connect and validate web search once a user-selected backend/key is available.
- Prepare an external Mac beta: signing, notarization, clean-machine installation, and onboarding.

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
- Realistic project and Auto output intent regressions cover explicit artifacts, informational questions, ambiguous projects, and weak new-folder signals.
- Private GitHub repository and macOS CI workflow created.
- 10 frontend and 51 Rust tests, frontend build, formatting, Clippy, and packaged app build pass. External distribution remains unsigned/unnotarized.
