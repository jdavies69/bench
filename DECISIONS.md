# Decisions

| Date | Decision | Rationale |
| --- | --- | --- |
| 2026-09-30 | Inspect macOS Keychain metadata for connection status; read secret data only when using the provider. | Opening Bench or refreshing Settings should not request password access. Keep credentials and their access controls intact; stable signing remains necessary across app updates. |
| 2026-09-30 | Permit only same-page section navigation in Website previews, explicitly bound to `about:srcdoc`. | Bare fragments inherit the application base URL and leave the preview; external destinations, scripts, and network access remain blocked. |
| 2026-09-30 | Reject Website filenames that differ only by capitalization before saving. | Default Mac filesystems can otherwise overwrite distinct generated pages and corrupt immutable revisions. |
| 2026-09-29 | Use Tauri 2, React/TypeScript, Rust, SQLite, and macOS Keychain. | Keep a native local workspace and credentials outside persisted frontend data. |
| 2026-09-29 | Separate model providers, tools, output modules, and action policy. | Adding one provider or output must not couple every integration to it. |
| 2026-09-29 | Default to Auto; implement Chat and static Website first. | Prove usable output before expanding breadth. Other outputs remain explicit placeholders. |
| 2026-09-29 | Preview static HTML/CSS with scripts and network access blocked; never execute generated commands. | Establish a small, understandable isolation boundary for the Website slice. |
| 2026-09-29 | Store Website revisions immutably and activate only completed revisions. | Failed generation or writes must preserve usable work. |
| 2026-09-29 | Group conservatively; Miscellaneous is the fallback and manual moves win. | Users should rarely file chats, and incorrect confident filing is costly. |
| 2026-09-29 | Initially planned configured Brave or SearXNG search. Superseded by the next decision. | The initial provider review missed OpenRouter's hosted search tool. |
| 2026-09-29 | Use OpenRouter's hosted web-search tool with the existing OpenRouter key as Bench's first search path. | OpenRouter executes provider-native search or a hosted fallback; users should not need a second search connection. Keep the generic tool boundary for future backends. |
| 2026-09-29 | Repository documentation and Git history are the development source of truth. | Future agents should continue verified work without requiring the user to manage every task. |
| 2026-09-29 | Defer accounts/billing; proposed managed usage is a monthly plan with an included hard cap. | Foundation quality comes first; no Bench-owned service accounts or host exist yet. |
| 2026-09-29 | Enforce Website actions in Rust with exact, expiring, one-use grants when review is required. | Persisted preferences must govern actions beyond UI copy or model prompts. |
| 2026-09-29 | Keep Website versions immutable and recover the newest validated version if the active pointer is damaged. | A failed generation or filesystem write must not destroy the last usable site. |
| 2026-09-29 | Allow 240 seconds for Website model requests while keeping Chat at 120 seconds. | A live revision exceeded the Chat timeout; the saved request and prior site remained intact, and the retry completed. |
| 2026-09-29 | Move BYOK configuration into Advanced when Bench-managed usage is functional. | Most users should not need provider setup, but hiding the only working inference path before managed mode exists would break onboarding. |
