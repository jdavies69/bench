use crate::{
    artifact::{ArtifactKind, ArtifactState},
    provider::{ModelProvider, ProviderMessage},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentContent {
    pub title: String,
    pub markdown: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Slide {
    pub title: String,
    pub body: String,
    pub notes: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationContent {
    pub title: String,
    pub slides: Vec<Slide>,
}

fn text(value: &str, max: usize, required: bool) -> bool {
    value.len() <= max && (!required || !value.trim().is_empty()) && !value.contains('\0')
}

pub fn validate(kind: ArtifactKind, content: &Value) -> Result<(), String> {
    let valid = match kind {
        ArtifactKind::Application => return crate::application::validate(kind, content),
        ArtifactKind::Document => {
            let document: DocumentContent = serde_json::from_value(content.clone())
                .map_err(|_| "Document response has an invalid format.")?;
            text(&document.title, 200, true) && text(&document.markdown, 200_000, true)
        }
        ArtifactKind::Presentation => {
            let deck: PresentationContent = serde_json::from_value(content.clone())
                .map_err(|_| "Presentation response has an invalid format.")?;
            text(&deck.title, 200, true)
                && !deck.slides.is_empty()
                && deck.slides.len() <= 20
                && deck.slides.iter().all(|slide| {
                    text(&slide.title, 200, true)
                        && text(&slide.body, 10_000, true)
                        && text(&slide.notes, 5_000, false)
                })
                && deck
                    .slides
                    .iter()
                    .map(|slide| slide.title.len() + slide.body.len() + slide.notes.len())
                    .sum::<usize>()
                    <= 200_000
        }
        _ => return Err("This output is not supported by the text generator.".into()),
    };
    if valid {
        Ok(())
    } else {
        Err("Output content is empty, invalid, or too large.".into())
    }
}

pub async fn generate(
    provider: &dyn ModelProvider,
    kind: ArtifactKind,
    previous: Option<&ArtifactState>,
    requests: &[String],
) -> Result<ArtifactState, String> {
    let schema = match kind {
        ArtifactKind::Application => {
            r#"{"title":"...","description":"...","fields":[{"id":"amount","label":"Amount","type":"number","default":0}],"outputs":[{"label":"Total","expression":{"op":"input","id":"amount"}}]}"#
        }
        ArtifactKind::Document => r#"{"title":"...","markdown":"..."}"#,
        ArtifactKind::Presentation => {
            r#"{"title":"...","slides":[{"title":"...","body":"...","notes":"..."}]}"#
        }
        _ => return Err("This output is not supported by the text generator.".into()),
    };
    if requests.is_empty() {
        return Err("Save a request before creating an output.".into());
    }
    if requests
        .iter()
        .fold(0_usize, |size, request| size.saturating_add(request.len()))
        > 400_000
    {
        return Err(
            "Output request history is too large. Start a new conversation with the current brief."
                .into(),
        );
    }
    if let Some(previous) = previous {
        if previous.kind != kind || previous.version != 1 {
            return Err("The current output has an incompatible format.".into());
        }
        validate(kind, &previous.content)?;
        if previous.request_count >= requests.len() {
            return Err(
                "This output is already up to date. Send a new request to revise it.".into(),
            );
        }
    }
    let system = format!("Create a polished {} for Bench. Return ONLY a JSON object with exactly this schema: {schema}. Return the full revised content, retaining existing material unless the user asks to change it. Never invent business facts, prices, testimonials, citations, or specific claims. Clearly label missing facts as placeholders. Use plain Markdown for documents and plain text for slide bodies and notes. A presentation has 1 to 20 slides. Applications are local forms/calculators/checklists. Field types number,text,select,toggle; select options string array, default exactly typed. Outputs expression AST op constant(value finite number), input(id numeric/toggle field), add/subtract/multiply/divide/min/max(left,right). Max30 fields20outputs, AST depth12 total256nodes, numbers abs<=1e12. No scripts, network, host commands or external integrations.", kind.as_str());
    let messages = vec![
        ProviderMessage { role: "system".into(), content: system },
        ProviderMessage { role: "user".into(), content: serde_json::json!({"requests":requests,"existing_content":previous.map(|state| &state.content)}).to_string() },
    ];
    let (tx, mut rx) = mpsc::unbounded_channel();
    let stream = provider.stream_chat(messages, tx);
    let collect = async {
        let mut output = String::new();
        while let Some(delta) = rx.recv().await {
            if output.len().saturating_add(delta.len()) > 400_000 {
                return Err("Output response is too large.".to_owned());
            }
            output.push_str(&delta);
        }
        Ok(output)
    };
    let (_, output) = futures_util::future::try_join(stream, collect).await?;
    let trimmed = output.trim();
    let json = if let Some(fenced) = trimmed
        .strip_prefix("```json\n")
        .or_else(|| trimmed.strip_prefix("```\n"))
    {
        fenced
            .strip_suffix("```")
            .ok_or("Output response was incomplete. Retry the request.")?
            .trim()
    } else {
        trimmed
    };
    let content: Value = serde_json::from_str(json)
        .map_err(|_| "Output response was incomplete. Retry the request.")?;
    validate(kind, &content)?;
    Ok(ArtifactState {
        version: 1,
        kind,
        revision: previous.map_or(Ok(1), |state| {
            state
                .revision
                .checked_add(1)
                .ok_or("Output history is full.")
        })?,
        request_count: requests.len(),
        content,
    })
}

pub fn export_text(state: &ArtifactState) -> Result<String, String> {
    validate(state.kind, &state.content)?;
    match state.kind {
        ArtifactKind::Document => {
            let document: DocumentContent = serde_json::from_value(state.content.clone())
                .map_err(|_| "Document is unreadable.")?;
            Ok(format!("# {}\n\n{}\n", document.title, document.markdown))
        }
        ArtifactKind::Presentation => serde_json::to_string_pretty(&state.content)
            .map_err(|_| "Presentation is unreadable.".into()),
        _ => Err("This output export is unavailable.".into()),
    }
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub fn export_html(state: &ArtifactState) -> Result<String, String> {
    validate(state.kind, &state.content)?;
    let (title, body) = match state.kind {
        ArtifactKind::Document => {
            let document: DocumentContent = serde_json::from_value(state.content.clone())
                .map_err(|_| "Document is unreadable.")?;
            (
                document.title.clone(),
                format!(
                    "<article><h1>{}</h1><div class=\"copy\">{}</div></article>",
                    escape(&document.title),
                    escape(&document.markdown)
                ),
            )
        }
        ArtifactKind::Presentation => {
            let deck: PresentationContent = serde_json::from_value(state.content.clone())
                .map_err(|_| "Presentation is unreadable.")?;
            let body = deck.slides.iter().enumerate().map(|(index, slide)| format!("<section><small>Slide {}</small><h2>{}</h2><div class=\"copy\">{}</div><aside><h3>Speaker notes</h3><div class=\"copy\">{}</div></aside></section>",index+1,escape(&slide.title),escape(&slide.body),escape(&slide.notes))).collect::<Vec<_>>().join("\n");
            (deck.title, body)
        }
        _ => return Err("This output export is unavailable.".into()),
    };
    Ok(format!("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'\"><title>{}</title><style>body{{margin:0;background:#f6f5f2;color:#252422;font:18px/1.6 system-ui,sans-serif}}main{{max-width:960px;margin:auto;padding:48px}}section{{padding:48px 0;break-after:page}}.copy{{white-space:pre-wrap;overflow-wrap:anywhere}}aside{{margin-top:32px;font-size:14px;color:#666}}@media print{{section{{min-height:85vh}}}}</style></head><body><main>{body}</main></body></html>",escape(&title)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact;
    use async_trait::async_trait;

    struct Fake {
        output: &'static str,
        failure: bool,
    }
    #[async_trait]
    impl ModelProvider for Fake {
        async fn stream_chat(
            &self,
            messages: Vec<ProviderMessage>,
            deltas: mpsc::UnboundedSender<String>,
        ) -> Result<(), String> {
            assert!(messages[0].content.contains("placeholders"));
            deltas.send(self.output.into()).unwrap();
            if self.failure {
                Err("Provider stopped before finishing".into())
            } else {
                Ok(())
            }
        }
    }
    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }
    const APP: &str = r#"{"title":"Form","description":"","fields":[{"id":"x","label":"X","type":"number","default":1}],"outputs":[]}"#;
    const DOC: &str = r##"{"title":"Brief","markdown":"# Overview\n\nA saved draft."}"##;
    const DECK: &str = r#"{"title":"Review","slides":[{"title":"First","body":"Overview","notes":"Introduce the topic"},{"title":"Second","body":"Next steps","notes":""}]}"#;

    #[test]
    fn generate_document_and_presentation_then_revise_and_reload() {
        let runtime = runtime();
        let root = std::env::temp_dir().join(format!("bench-text-{}", uuid::Uuid::new_v4()));
        let id = uuid::Uuid::new_v4().to_string();
        for (kind, output) in [
            (ArtifactKind::Document, DOC),
            (ArtifactKind::Presentation, DECK),
            (ArtifactKind::Application, APP),
        ] {
            let first = runtime
                .block_on(generate(
                    &Fake {
                        output,
                        failure: false,
                    },
                    kind,
                    None,
                    &["Create".into()],
                ))
                .unwrap();
            let saved = artifact::save(&root, &id, &first, validate).unwrap();
            let revised = runtime
                .block_on(generate(
                    &Fake {
                        output,
                        failure: false,
                    },
                    kind,
                    Some(&saved),
                    &["Create".into(), "Revise".into()],
                ))
                .unwrap();
            let revised = artifact::save(&root, &id, &revised, validate).unwrap();
            let relaunched = artifact::load(&root, &id, kind, validate).unwrap().unwrap();
            assert_eq!(relaunched.revision, 2);
            assert_eq!(relaunched.request_count, 2);
            assert_eq!(relaunched.content, revised.content);
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn application_partial_or_invalid_revision_preserves_last_good() {
        let runtime = runtime();
        let root = std::env::temp_dir().join(format!("bench-application-{}", uuid::Uuid::new_v4()));
        let id = uuid::Uuid::new_v4().to_string();
        let first = runtime
            .block_on(generate(
                &Fake {
                    output: APP,
                    failure: false,
                },
                ArtifactKind::Application,
                None,
                &["Create".into()],
            ))
            .unwrap();
        let first = artifact::save(&root, &id, &first, validate).unwrap();
        for (output, failure) in [(APP, true), ("{broken", false), ("{}", false)] {
            assert!(runtime
                .block_on(generate(
                    &Fake { output, failure },
                    ArtifactKind::Application,
                    Some(&first),
                    &["Create".into(), "Revise".into()]
                ))
                .is_err());
            let saved = artifact::load(&root, &id, ArtifactKind::Application, validate)
                .unwrap()
                .unwrap();
            assert_eq!(saved.revision, 1);
            assert_eq!(saved.request_count, 1);
            assert_eq!(saved.content, first.content);
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_malformed_truncated_and_oversized_generations_preserve_last_good() {
        let runtime = runtime();
        let root = std::env::temp_dir().join(format!("bench-text-{}", uuid::Uuid::new_v4()));
        let id = uuid::Uuid::new_v4().to_string();
        let initial = runtime
            .block_on(generate(
                &Fake {
                    output: DOC,
                    failure: false,
                },
                ArtifactKind::Document,
                None,
                &["Create".into()],
            ))
            .unwrap();
        let initial = artifact::save(&root, &id, &initial, validate).unwrap();
        for (output, failure) in [
            ("", false),
            ("{broken", false),
            ("{}", false),
            (r#"{"title":"Empty","markdown":""}"#, false),
            (
                r#"{"title":"Wrong","markdown":"body","command":"rm"}"#,
                false,
            ),
            (DOC, true),
            ("```json\n{}", false),
        ] {
            let result = runtime.block_on(generate(
                &Fake { output, failure },
                ArtifactKind::Document,
                Some(&initial),
                &["Create".into(), "Revise".into()],
            ));
            assert!(result.is_err());
            let saved = artifact::load(&root, &id, ArtifactKind::Document, validate)
                .unwrap()
                .unwrap();
            assert_eq!(saved.content, initial.content);
            assert_eq!(saved.request_count, 1);
        }
        struct Oversize;
        #[async_trait]
        impl ModelProvider for Oversize {
            async fn stream_chat(
                &self,
                _: Vec<ProviderMessage>,
                deltas: mpsc::UnboundedSender<String>,
            ) -> Result<(), String> {
                deltas.send("x".repeat(400_001)).unwrap();
                Ok(())
            }
        }
        assert!(runtime
            .block_on(generate(
                &Oversize,
                ArtifactKind::Document,
                Some(&initial),
                &["Create".into(), "Revise".into()]
            ))
            .unwrap_err()
            .contains("too large"));
        assert!(runtime
            .block_on(generate(
                &Fake {
                    output: DOC,
                    failure: false
                },
                ArtifactKind::Document,
                Some(&initial),
                &["Create".into(), "x".repeat(400_001)],
            ))
            .unwrap_err()
            .contains("history is too large"));
        assert_eq!(
            artifact::list_revisions(&root, &id, ArtifactKind::Document, validate)
                .unwrap()
                .len(),
            1
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn strict_slide_schema_and_bounds_reject_unusable_decks() {
        for content in [
            serde_json::json!({"title":"Deck","slides":[]}),
            serde_json::json!({"title":"Deck","slides":[{"title":"One","body":"","notes":""}]}),
            serde_json::json!({"title":"Deck","slides":[{"title":"One","body":"body"}]}),
            serde_json::json!({"title":"Deck","slides":[{"title":"One","body":"body","notes":"","url":"https://example.com"}]}),
        ] {
            assert!(validate(ArtifactKind::Presentation, &content).is_err());
        }
        let slide = serde_json::json!({"title":"One","body":"body","notes":""});
        assert!(validate(
            ArtifactKind::Presentation,
            &serde_json::json!({"title":"Deck","slides":vec![slide;21]})
        )
        .is_err());
    }

    #[test]
    fn exports_escape_active_content_and_preserve_slide_order_and_notes() {
        let mut document = runtime()
            .block_on(generate(
                &Fake {
                    output: DOC,
                    failure: false,
                },
                ArtifactKind::Document,
                None,
                &["Create".into()],
            ))
            .unwrap();
        document.content = serde_json::json!({"title":"<script>title</script>","markdown":"<img src=https://example.com onerror=alert(1)> & text"});
        let html = export_html(&document).unwrap();
        assert!(!html.contains("<script>"));
        assert!(!html.contains("<img src="));
        assert!(html.contains("&lt;img"));
        assert!(html.contains("default-src 'none'"));
        assert!(export_text(&document)
            .unwrap()
            .starts_with("# <script>title</script>"));
        let deck = runtime()
            .block_on(generate(
                &Fake {
                    output: DECK,
                    failure: false,
                },
                ArtifactKind::Presentation,
                None,
                &["Create".into()],
            ))
            .unwrap();
        let html = export_html(&deck).unwrap();
        assert!(html.find("First").unwrap() < html.find("Second").unwrap());
        assert!(html.contains("Introduce the topic"));
        assert_eq!(
            serde_json::from_str::<Value>(&export_text(&deck).unwrap()).unwrap(),
            deck.content
        );
    }
}
