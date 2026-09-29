mod web_search;

use async_trait::async_trait;
use serde::Serialize;

pub use web_search::{requires_current_info, WebSearch};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResult {
    pub query: String,
    pub backend: String,
    pub elapsed_ms: u128,
    pub summary: String,
    pub results: Vec<SearchHit>,
}

impl ToolResult {
    /// Search text is untrusted external content. Callers should also tell the model
    /// to treat it as evidence, not instructions.
    pub fn for_model(&self) -> String {
        let mut text = format!("Web search results for {:?}:\n", self.query);
        for (index, hit) in self.results.iter().enumerate() {
            text.push_str(&format!(
                "{}. {}\nURL: {}\nSnippet: {}\n",
                index + 1,
                hit.title,
                hit.url,
                hit.snippet
            ));
        }
        text
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolError {
    pub kind: &'static str,
    pub message: &'static str,
}

impl ToolError {
    pub const fn new(kind: &'static str, message: &'static str) -> Self {
        Self { kind, message }
    }
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    /// The implementation owns its category; providers and UI cannot lower it.
    fn category(&self) -> crate::policy::ActionCategory;
    async fn execute(&self, query: &str) -> Result<ToolResult, ToolError>;
}
