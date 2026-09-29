/// Tool identity and authorization stay separate from the selected model
/// provider. Adapters decide how to execute each capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolKind {
    WebSearch,
}

impl ToolKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::WebSearch => "web_search",
        }
    }

    pub fn category(self) -> crate::policy::ActionCategory {
        match self {
            Self::WebSearch => crate::policy::ActionCategory::ReadOnly,
        }
    }
}

pub fn requires_current_info(query: &str) -> bool {
    let text = query.to_lowercase();
    let current_words = [
        "current",
        "currently",
        "latest",
        "today",
        "tonight",
        "tomorrow",
        "this week",
        "this month",
        "right now",
        "live",
        "recent",
        "as of",
        "up to date",
        "up-to-date",
    ];
    let volatile_topics = [
        "weather",
        "forecast",
        "stock price",
        "share price",
        "price of",
        "market cap",
        "exchange rate",
        "news",
        "announcement",
        "announced",
        "earnings",
        "score",
        "schedule",
        "who is the",
        "what is the price",
    ];
    let asks_for_facts = [
        "what",
        "who",
        "when",
        "where",
        "how",
        "tell me",
        "show me",
        "give me",
        "check",
        "find",
        "search",
        "weather in",
        "forecast for",
        "stock price",
        "share price",
    ]
    .iter()
    .any(|word| contains_term(&text, word));
    current_words.iter().any(|word| contains_term(&text, word))
        || (asks_for_facts
            && volatile_topics
                .iter()
                .any(|word| contains_term(&text, word)))
}

fn contains_term(text: &str, term: &str) -> bool {
    text.match_indices(term).any(|(start, _)| {
        let before = text[..start].chars().next_back();
        let after = text[start + term.len()..].chars().next();
        !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_current_requests() {
        assert!(requires_current_info("weather in NYC this week"));
        assert!(requires_current_info("latest OpenAI announcement"));
        assert!(requires_current_info("current NVIDIA price"));
        assert!(!requires_current_info("Build a website for Laundros"));
        assert!(!requires_current_info("Build a weather app"));
        assert!(!requires_current_info("Deliver the document"));
        assert!(!requires_current_info("Explain how a database index works"));
    }

    #[test]
    fn tool_identity_and_policy_are_provider_independent() {
        assert_eq!(ToolKind::WebSearch.name(), "web_search");
        assert_eq!(
            ToolKind::WebSearch.category(),
            crate::policy::ActionCategory::ReadOnly
        );
    }
}
