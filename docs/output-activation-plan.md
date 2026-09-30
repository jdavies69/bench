# Activating all output modules

Status: implementation plan. The user explicitly requested all currently listed outputs to work, superseding the earlier breadth deferral. A listed output is complete only when it generates a real artifact or executes a real bounded workflow, supports revision/recovery, and survives restart. Fake-provider tests prove behavior without spending; paid live acceptance remains separately authorized.

## Shared architecture first

`ModelProvider` currently exposes text streaming only. Keep that path for Document, Presentation, Application, and planning; add separate image/speech capabilities rather than interpreting binary media as chat. Existing OpenRouter credentials can serve media without a second connection. Unsupported selected providers need a clear capability error that preserves the request; never silently switch billing to another user's connection.

The workspace registry currently accepts Website-only props. Replace that with a discriminated output state union and per-output workspace modules. Rust remains authoritative for output type, action category, bounded files, revisions, and grants. Extend immutable artifact storage with validated content types and last-good activation; reuse the existing Website policy patterns instead of adding frontend filesystem writes. Keep shared `App.tsx`, `native.ts`, `commands.rs`, `output.rs`, `db.rs`, and registry integration under the lead's ownership.

## Smallest complete implementation

| Output | Working first slice | Required acceptance |
| --- | --- | --- |
| Document | Generate bounded Markdown; native read view and editable content; conversational revision; export Markdown and standalone safe HTML | Real artifact, revision preservation, unknown claims left as placeholders, restart/edit/export round trip, failed generation leaves last good document |
| Presentation | Generate validated JSON slide deck; native slide renderer/navigation; edit title/body/notes; conversational revision; export deck JSON and standalone script-free HTML | Bounded slide count/content, malformed schema rejection, keyboard navigation, correct export content/order, restart and last-good recovery; no PPTX claim |
| Image | OpenRouter image generation; local validated raster bytes; zoom/fit; save/export exact image; revision as another immutable generation, references only when supported | Bounded base64/decoded sizes, byte-format verification, reject SVG/HTML and remote URLs initially, corrupt/empty media rejection, failed revision preserves image, exact export and restart |
| Voice | Generate/revise a visible script, synthesize selected supported voice to MP3; native playback and export | No automatic microphone access/cloning; explicit synthesis; bounded script/audio, MIME/container validation, native playback, retry/restart, failed synthesis preserves prior audio |
| Application | Generate a self-contained local HTML/CSS/JS interactive app; sandboxed preview, reload/reset, revisions; export self-contained file | Test actual interaction, persistence of source, failure recovery; scripts cannot reach host IPC/storage, navigate top window, fetch network, or import remote code; export clearly retains sandbox requirements |
| Agent | Persist a bounded execution plan and journal; execute real allowlisted artifact read/create/revise tools sequentially under Rust approval; show produced artifacts and step results | No generic host commands, browser automation, recurring schedule or integrations claimed; validate tool/argument schemas, exact per-step authorization, max steps/calls/context, cancel/fail/resume, no replay of completed writes after restart |

Document means a usable Markdown/HTML document in this slice; Presentation means a usable slide deck in the native viewer and HTML export. DOCX/PPTX/PDF can follow if specifically needed, but must not appear as working exports before implemented. Export outside app data requires a user-selected destination and overwrite protection; preserve the same Rust authorization boundary.

Agent must perform tools and produce artifacts; a chat answer containing a plan does not count. A concrete initial workflow can inspect an existing artifact, produce a brief Document, and create/revise a Website with recorded tool results. The model chooses only from typed allowlisted actions; Rust reconstructs grants against the current step, request, and artifact revision. Uncertain execution results must remain pending until recovered, not run twice.

Application needs a distinct preview security boundary. Do not relax Website's script-free sanitizer or enable same-origin in a generated-script iframe. Child CSP alone does not establish that self-navigation cannot contact a remote URL; add/test a parent navigation/frame restriction too. Opaque sandbox origin, no host bridge, no accepted arbitrary postMessage commands, and no remote resource loading are mandatory. Arbitrary generated JS can loop or consume resources even in a sandbox; measure responsiveness and establish a recovery path before calling it safe. A preview is a local interactive application, not a native macOS binary or deployed service.

## Provider facts verified September 30

[OpenRouter's current Image API](https://openrouter.ai/docs/guides/overview/multimodal/image-generation) documents `POST /api/v1/images` with model and prompt, and base64 image bytes in `data[].b64_json` with `media_type`. Discover capabilities through `/api/v1/images/models` and per-model endpoints. Use supported parameters for the selected endpoint; request one raster image initially. This is a dedicated path, separate from the existing chat stream parser.

[OpenRouter's TTS API](https://openrouter.ai/docs/guides/overview/multimodal/tts) documents `POST /api/v1/audio/speech` with model, input, supported voice, and `response_format: "mp3"`; its response is raw audio bytes, not JSON. Models/voices vary; select from verified speech capabilities. Request MP3 explicitly because the documented default PCM lacks a directly usable file container. Do not introduce voice cloning in this slice.

Model selection and specific paid acceptance are pending. Do not assume the configured text model generates images or speech, and do not infer a model's current endpoint/price from a historic name. Mock media transport can complete storage/playback/export tests independently of those choices.

## Independent ownership and integration order

1. Artifact foundation + Document/Presentation Rust module: one agent owns new `artifact.rs`/`text_outputs.rs` with validation, immutable revisions, fake-provider generation and recovery tests. Lead integrates shared commands/policy/DB.
2. Document/Presentation workspaces: one agent owns new React modules and tests against the agreed discriminated state contracts. Lead integrates registry/native/App.
3. Image/Voice transport + media storage: one agent owns a new Rust media provider/module and fixture tests; separate UI modules can follow the typed result contract. Model/voice metadata must remain explicit and verified.
4. Application preview + Agent engine: implement after shared artifact storage and typed policy hooks exist. Keep Application security tests independent from Website. Agent engine owns its persisted journal and tool schema, while the lead wires existing artifact tools.

Land coherent output milestones, then run frontend/Rust validation and packaged native acceptance. Track generated, revised, exported, restart-tested, native-reviewed, and live-paid-tested separately per output. All six outputs remain unfinished until their concrete acceptance is met; enabling menu entries alone is insufficient.
