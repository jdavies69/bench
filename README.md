# Bench

Bench is a free, MIT-licensed open-source desktop AI workspace built with Tauri 2, React, TypeScript, Rust, and SQLite. It supports streaming chat, local search, automatic project grouping, manual project moves, output routing, static Website workspaces, and persistent settings.

## Download (development build)

[Download Bench for Apple Silicon](https://github.com/jdavies69/bench/releases/latest). This free development build is ad-hoc signed and **not Apple-notarized**; macOS may require explicit first-launch approval through Privacy & Security. Do not disable Gatekeeper globally. Tested on macOS 27 Apple Silicon; older macOS and Intel compatibility are unverified. Open the DMG and drag Bench into Applications once. Subsequent releases download inside Bench; Settings offers Restart to update.

## Run

Install Node dependencies with `npm install`, then run `npm run tauri dev`.

Open **Settings → Connections → OpenRouter** and choose **Connect OpenRouter**. Manual API-key entry is also available; model settings live under **Settings → Advanced**.

A fresh disconnected workspace offers **Connect OpenRouter**. Bench opens browser sign-in using OAuth PKCE, receives a one-time localhost callback, and stores the connection credential directly in macOS Keychain. No key copy/paste or Bench account is required. Cancel or set up later; **Settings → Connections → OpenRouter → Connect OpenRouter** reopens setup. Manual key entry remains a Settings fallback. Connecting makes no model request; OpenRouter bills subsequent usage directly.

**Settings → Usage & billing → Check usage** loads native spend and limit metrics for the connected OpenRouter key. It never fetches on app launch or Settings entry. Metrics include all apps sharing that key; the key’s remaining allowance is not the account’s credit balance. Manage credits still opens OpenRouter.

Keys are stored in the system credential store (macOS Keychain on macOS). The password field sends the key once to Rust through Tauri IPC and clears after saving; React does not persist keys or call model APIs. SQLite stores only provider and model preferences. Existing `OPENROUTER_API_KEY`, `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, and `XAI_API_KEY` launch environment variables also work as fallbacks. Without a key for the selected provider, local conversations still save and the app shows a clear error when sending.

## Workspaces and local data

On macOS, startup and connection status inspect Keychain metadata without reading API-key passwords. Protected password access happens when a model request actually needs the key. Ad-hoc development rebuilds can still trigger approval on that first use because their signing identity changes; stable signing is required for dependable upgrades. Keys and their access controls remain in Keychain.

SQLite lives in Tauri's app data directory under the `app.bench.desktop` identifier. Projects, conversations, messages, output intent, sidebar preference, tool metadata, and settings stay on the device. Bench assigns a project when a conversation clearly names one, and can update an uncertain assignment as more context arrives. Uncertain or ambiguous conversations go to Miscellaneous. A manual project move stays fixed.

`Auto` classifies a new request. Chat works end to end. Website creates and revises static `index.html`, `style.css`, and optional additional HTML pages in a per-conversation app-data workspace. The preview runs in a sandboxed iframe with scripts blocked; Bench does not run generated commands. Document, Presentation, Image, Voice, and bounded declarative Application workflows now support generation, revisions, persistence, and export. Application supports forms/calculators, not arbitrary generated programs. Agent supports bounded local conversation synthesis with read/draft/inspect tool rounds; it does not browse or run commands. Automated fake-provider tests pass; paid live checks remain pending. The conversation stays available beside an output workspace, which can be closed and reopened.

Generated Website revisions are stored under `website-workspaces/<conversation-id>/revision-<number>`. Bench activates a revision only after its files are written and validated. Revisions patch the existing files, preserve previous versions, and keep the last usable preview on a failed generation. The Website workspace's **Versions** control lets you restore a saved version. Generated filenames are restricted to top-level HTML files and `style.css`; active HTML content is rejected.

The isolated preview supports links to sections on the same page; scripts, external navigation, and network resources remain blocked. Filenames that differ only by capitalization are rejected before saving to protect revisions on case-insensitive Mac filesystems.

Generated copy is a draft. Bench asks the model to leave unknown business details as placeholders; verify all claims before publishing. The first live Laundros test was corrected locally to replace invented contact details, hours, and prices without another model call.

## Web search

With OpenRouter selected, Bench offers OpenRouter's hosted `openrouter:web_search` tool to Chat using the existing OpenRouter key. The model can search when it needs current information. Bench caps server-tool calls, records search activity with the saved user message, and requires a valid source citation before saving an answer to a clearly current-information request. **Settings → Advanced → Web search** can turn it off. With another model provider selected, Bench cannot verify current facts through this tool and asks you to select OpenRouter for such requests. The generic tool identity and authorization remain separate from the model-provider implementation.

Older Brave and SearXNG settings migrate to OpenRouter search. Their stored Keychain credential and URL are preserved locally but no longer used. The OpenRouter streaming search path has mock coverage; a live search has not yet been verified.

Execution and approval preferences are enforced in Rust for Website creation, revision, and version restoration. **Discuss first** or **Always ask** requires an explicit inline approval; approvals authorize one exact request and revision, expire, and cannot be reused. **Balanced** permits directly requested reversible local writes while reviewing model-initiated changes. Sensitive, destructive, and financial categories remain approval boundaries for future tools. Chat itself does not execute write actions.

## Verify

- `npm run build`
- `npm test`
- From `src-tauri`: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets --all-features -- -D warnings`, and `cargo test --locked --all-targets --all-features`
- `npm run tauri build -- --bundles app`

Streaming can be exercised end to end once a valid provider key is saved. The local test suite covers output intent, project grouping and manual reassignment, stream event parsing, error mapping, website creation and revision using a fake provider, hosted-search request and citation parsing, failed SQLite writes, and reopening the database. Live web search requires a connected OpenRouter key and remains unverified.

CI builds the frontend and runs frontend and beta-checker tests, Rust formatting, Clippy, and tests on macOS. No API credentials are needed. Read `PRODUCT.md`, `ROADMAP.md`, `DECISIONS.md`, and `AGENTS.md` before development. Public downloads are explicitly labeled development artifacts; Developer ID signing, notarization, and clean-machine installation remain pending for a polished release. See [external Mac beta preparation](docs/macos-beta.md) for the read-only artifact checker and acceptance steps.

## App updates

Signed updater support verifies the package and its announced version before installation. Settings supports automatic download and an explicit restart to install. The development feed is configured at GitHub Releases. Packages and announced versions are verified with Bench’s updater key; publication and native acceptance evidence are tracked in ROADMAP.md. The updater signing key is separate from Apple Developer ID signing.

## File attachments

Click **+**, choose files in the Mac file picker, review the filename chips, write a request, and send. Remove a chip to exclude that file. Choosing a file does not contact a model. Sending includes selected text/images with the request; attachments persist locally with the saved message, including failed responses and retries.

Supported: PNG/JPEG/WebP (5 MiB each), TXT/MD/CSV/JSON, DOCX, XLSX, and supported text PDFs (10 MiB document source). Up to five files, 20 MiB total; extracted text is limited to 256 KiB per file and 1 MiB per message. Image inputs require an OpenRouter model that supports images; incompatible models fail explicitly. Image output uses uploads as actual image-generation references. Spreadsheets supply saved cell values, not executed formulas or macros. Legacy XLS needs XLSX/CSV conversion.

PDF extraction is conservative: classic cross-reference tables and bounded Flate streams are supported. Encrypted PDFs, compressed object/cross-reference streams, unsupported filters, scans requiring OCR, and malformed or excessively expanded content are rejected with an explanation. DOCX/XLSX archives and extracted content are bounded; generated or embedded code is never executed.
