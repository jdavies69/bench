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
- Application, Presentation, Document, Image, Agent, and Voice retain requests in explicit placeholder workspaces.

## Boundaries

SQLite owns local product data. Rust owns providers, tools, actions, and authorization. macOS Keychain owns credentials. Output modules are independent from providers and share one adaptive workspace.

Website quality and dependable local behavior come before output breadth. Preview section links stay inside the current page, and conflicting Mac filenames are rejected before saving revisions. macOS connection status uses Keychain metadata; opening the app does not need to read API-key passwords. Accounts, billing, cloud sync, organizations, mobile, marketplace, deployment, and new service integrations are deferred. Rust currently enforces execution and approval preferences for Website actions; new action types must pass through the same policy boundary.

When Bench-managed usage exists, ordinary users should not need to see provider keys or model setup. Keep BYOK available in Advanced for users who choose it. Until managed usage works, the connection path must remain discoverable so a fresh install can run Chat.
