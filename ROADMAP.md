# Roadmap

## Now

- Live-verify OpenRouter's hosted web-search stream and citations after a specific paid-call authorization.
- Visually review the **current installed build** in Computer Use: blank chat, active conversation, expanded/collapsed sidebar, and Settings. The previous visual check reached an older installed build and must not count as verification of the current UI.
- Review the Website output for usability and factual accuracy across more live revisions.

## Current handoff

- The installed app was built from code commit `adb2334`, whose GitHub CI passed. That source was rebuilt on September 30 and installed at `/Applications/Bench.app`; its bundle matches `src-tauri/target/release/bundle/macos/Bench.app`. The previous installation is temporarily backed up at `/private/tmp/Bench-prior-20260930.app`. Later documentation-only commits do not change the app build.
- Computer Use connected to the older app on September 30, then lost its native connection. Its helper logged repeated `Sender process is not authenticated` errors. Restarting the helper and connector did not restore visual access in that session. After reopening ChatGPT, retry Computer Use against `/Applications/Bench.app` before drawing UI conclusions. Bench itself launched as a process; the updated window was **not** visually verified.
- A live OpenRouter web-search test has **not** been authorized or run. The earlier approval to run one live Website test does not cover search. Ask for specific authorization before a paid model/search request, then check citations and failure handling in the installed build.

## Next

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
- OpenRouter-hosted web search uses the existing key, with capped tool calls, source checks for current answers, and preserved legacy credentials. Mock tests pass; live behavior awaits verification.
- Realistic project and Auto output intent regressions cover explicit artifacts, informational questions, ambiguous projects, and weak new-folder signals.
- Private GitHub repository and macOS CI workflow created.
- 12 frontend and 57 Rust tests, frontend build, formatting, Clippy, packaged app build, and GitHub CI passed for `adb2334`. External distribution remains unsigned/unnotarized.
