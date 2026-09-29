# Bench v0.0.1

Bench is a local desktop AI workspace built with Tauri 2, React, TypeScript, Rust, and SQLite. It supports streaming chat, local search, automatic project grouping, manual project moves, output routing, static Website workspaces, and persistent settings.

## Run

Install Node dependencies with `npm install`, then run `npm run tauri dev`.

Open **Settings → Connections**, choose OpenRouter, OpenAI, Anthropic, or xAI / Grok, paste that provider's API key, and select **Connect**. Saving a key makes its provider active for chat. You can save more than one key and switch providers with **Use for chat**. Explicit model IDs live under **Settings → Advanced**.

Keys are stored in the system credential store (macOS Keychain on macOS). The password field sends the key once to Rust through Tauri IPC and clears after saving; React does not persist keys or call model APIs. SQLite stores only provider and model preferences. Existing `OPENROUTER_API_KEY`, `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, and `XAI_API_KEY` launch environment variables also work as fallbacks. Without a key for the selected provider, local conversations still save and the app shows a clear error when sending.

## Workspaces and local data

SQLite lives in Tauri's app data directory under the `app.bench.desktop` identifier. Projects, conversations, messages, output intent, sidebar preference, tool metadata, and settings stay on the device. Bench assigns a project when a conversation clearly names one, and can update an uncertain assignment as more context arrives. Uncertain or ambiguous conversations go to Miscellaneous. A manual project move stays fixed.

`Auto` classifies a new request. Chat works end to end. Website creates and revises static `index.html`, `style.css`, and optional additional HTML pages in a per-conversation app-data workspace. The preview runs in a sandboxed iframe with scripts blocked; Bench does not run generated commands. Application, Presentation, Document, Image, Agent, and Voice currently open clear placeholder workspaces. The conversation stays available beside an output workspace, which can be closed and reopened.

Generated Website revisions are stored under `website-workspaces/<conversation-id>/revision-<number>`. Bench activates a revision only after its files are written and validated. Revisions patch the existing files, preserve previous versions, and keep the last usable preview on a failed generation. The Website workspace's **Versions** control lets you restore a saved version. Generated filenames are restricted to top-level HTML files and `style.css`; active HTML content is rejected.

Generated copy is a draft. Bench asks the model to leave unknown business details as placeholders; verify all claims before publishing. The first live Laundros test was corrected locally to replace invented contact details, hours, and prices without another model call.

## Web search

Web search is a separate Rust tool, independent of the selected model provider. In **Settings → Advanced → Web search**, connect a Brave Search API key or enter the URL of a JSON-enabled SearXNG instance, then save the source. Brave's key is held in the system credential store; SQLite stores only the source choice and SearXNG URL. `BRAVE_SEARCH_API_KEY` and `BENCH_SEARXNG_URL` in Bench's launch environment remain fallbacks under **Use available**. Search results are passed back into the model turn with source URLs, and execution metadata is saved locally. Without a search connection, Bench declines to verify current facts instead of guessing. Model API keys do not automatically provide web search.

Execution and approval preferences are enforced in Rust for Website creation, revision, and version restoration. **Discuss first** or **Always ask** requires an explicit inline approval; approvals authorize one exact request and revision, expire, and cannot be reused. **Balanced** permits directly requested reversible local writes while reviewing model-initiated changes. Sensitive, destructive, and financial categories remain approval boundaries for future tools. Chat itself does not execute write actions.

## Verify

- `npm run build`
- `npm test`
- From `src-tauri`: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets --all-features -- -D warnings`, and `cargo test --locked --all-targets --all-features`
- `npm run tauri build -- --bundles app`

Streaming can be exercised end to end once a valid provider key is saved. The local test suite covers output intent, project grouping and manual reassignment, stream event parsing, error mapping, website creation and revision using a fake provider, tool result parsing, failed SQLite writes, and reopening the database. Live web search requires a configured search backend.

CI builds the frontend and runs frontend tests, Rust formatting, Clippy, and tests on macOS. No API credentials are needed. Read `PRODUCT.md`, `ROADMAP.md`, `DECISIONS.md`, and `AGENTS.md` before development. Packaged builds are local development artifacts until Developer ID signing, notarization, and clean-machine installation are verified.
