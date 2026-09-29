# Working on Bench

Read `PRODUCT.md`, `ROADMAP.md`, and `DECISIONS.md` at the start of each cycle. Inspect Git status and preserve existing work. Update these documents when verified behavior or significant decisions change; keep them concise.

## Product and architecture

- Preserve the quiet native Mac experience: warm dot grid, floating palette, sparse composer, neutral tokens, clean content. No dashboards, suggested prompts, decorative AI icons, or model configuration in the primary flow.
- Rust owns providers, tools, filesystem operations, and enforced policy. React owns presentation and transient interaction. SQLite owns local structured data; Keychain owns credentials. Never log, commit, or persist API keys in frontend/storage files.
- Keep model providers, tools, and outputs independent. Extend one shared adaptive workspace. Unsupported outputs must honestly retain requests without fake functionality.
- Generated Website code is untrusted. Keep script/network isolation, restrict paths and sizes, preserve last known-good revisions, and never execute generated host commands.
- Authorization belongs in Rust. UI approval must identify the exact action; do not trust a model's claim of authorization or a frontend-supplied action category.

## Coordination

- Lead agent selects the highest-value unfinished milestone and continues useful work without routine permission requests.
- Delegate truly independent scopes in parallel. Assign file ownership before edits; route shared-file changes through the lead. Review every contribution before integration.
- Ask for consequential product choices, missing credentials, paid service selection, risky architectural forks, or irreversible external actions. Do ordinary engineering work autonomously.
- Paid model calls need specific authorization. Preserve the user's existing keys and conversations. Use fake providers and temporary workspaces for automated tests.

## Validation and delivery

- Run `npm run build`; add meaningful frontend tests when behavior warrants them.
- Run `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features` from `src-tauri` after integration.
- Build the packaged app for native/release changes and visually inspect affected states. Do not equate a build with live functional verification or distribution readiness.
- Test failure recovery, restart persistence, and authorization boundaries for actions and generated output.
- Keep secrets, local databases, generated user workspaces, dependencies, and build artifacts out of Git. Commit coherent verified milestones and keep the remote current.
- Report what changed, what passed, and concrete remaining blockers. Keep `ROADMAP.md` current and continue the next sensible task until a real decision or access boundary requires the user.
