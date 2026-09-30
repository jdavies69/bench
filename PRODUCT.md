# Bench

Bench turns an idea into a usable output in a quiet native Mac workspace.

## Experience

- Begin with a warm off-white dot-grid canvas, floating tool palette, and centered composer. Auto is the default output selection.
- Choose outputs through a compact neutral list with a selected checkmark; unfinished outputs sit under Coming later and retain requests in placeholder workspaces.
- Let the workspace emerge from the request. Chat uses a conversation; Website uses a conversation beside a preview. Keep the conversation available during revisions.
- Organize conversations automatically. Confident matches enter existing projects; uncertain matches enter Miscellaneous. Manual reassignment takes precedence.
- Use restrained motion, neutral colors, consistent design tokens, generous whitespace, and clean Markdown. Avoid dashboards, suggestions, decorative AI imagery, unnecessary borders, and technical configuration in the primary workflow.
- Keep prompts and local work safe through failures. Show concise inline recovery actions.

## Supported today

- Local streaming Chat with OpenRouter, OpenAI, Anthropic, and xAI API keys.
- Static Website generation and conversational file revisions in isolated local workspaces; script-free preview beside the conversation.
- Local projects, conversations, search, and settings. OpenRouter Chat can use hosted web search with the existing OpenRouter key; the tool identity and authorization remain provider-independent.
- Connect OpenRouter through browser sign-in (OAuth PKCE), with automatic return and credential storage in Keychain. Setup can be skipped or reopened from Settings; manual OpenRouter key entry remains available. Settings exposes only OpenRouter; legacy provider credentials remain intact. Connecting does not make a model request.
- Native OpenRouter usage in Settings: manual refresh shows spend and the connected key’s limit. These figures cover all apps using that key, not an account credit balance.
- Document and Presentation generation/revision with local editing and export; Image generation/revision and raster export; Voice drafting, audio playback, and export. Application supports bounded local forms/calculators with saved inputs and interactive HTML export. These workflows pass fake-provider tests; paid live generation verification is pending. Agent performs bounded local conversation synthesis through actual read/draft/inspect tool rounds and saves a report only on successful completion. It does not browse, run commands, or access arbitrary files.
- Signed updater verification and native Settings controls are implemented. Automatic delivery is pending a configured public release feed and an in-place upgrade test.

## Boundaries

SQLite owns local product data. Rust owns providers, tools, actions, and authorization. macOS Keychain owns credentials. Output modules are independent from providers and share one adaptive workspace.

Activate the existing output types with dependable generation, persistence, and useful export/playback; unfinished outputs remain explicitly marked. Preview section links stay inside the current page, and conflicting Mac filenames are rejected before saving revisions. macOS connection status uses Keychain metadata; opening the app does not need to read API-key passwords. The first public release targets Apple Silicon with users' own API keys. Accounts, billing, cloud sync, organizations, mobile, marketplace, deployment, and new service integrations remain deferred. Rust currently enforces execution and approval preferences for Website actions; new action types must pass through the same policy boundary.

When Bench-managed usage exists, ordinary users should not need to see provider keys or model setup. Keep BYOK available in Advanced for users who choose it. Until managed usage works, the connection path must remain discoverable so a fresh install can run Chat.
