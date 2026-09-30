//! Output intent routing. This stays separate from model and tool selection.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Implementation {
    Chat,
    Website,
    Artifact,
    Deferred,
}

/// A module declares how it joins the common workspace before it implements work.
/// Provider choice and tool execution remain separate capabilities.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputDefinition {
    pub id: &'static str,
    pub label: &'static str,
    pub implemented: bool,
    pub workspace: &'static str,
    #[serde(skip)]
    pub implementation: Implementation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputType {
    Chat,
    Website,
    Application,
    Presentation,
    Document,
    Image,
    Agent,
    Voice,
}

impl OutputType {
    pub const ALL: [Self; 8] = [
        Self::Chat,
        Self::Website,
        Self::Application,
        Self::Presentation,
        Self::Document,
        Self::Image,
        Self::Agent,
        Self::Voice,
    ];

    pub fn definition(self) -> OutputDefinition {
        let (label, implementation) = match self {
            Self::Chat => ("Chat", Implementation::Chat),
            Self::Website => ("Website", Implementation::Website),
            Self::Application => ("Application", Implementation::Artifact),
            Self::Presentation => ("Presentation", Implementation::Artifact),
            Self::Document => ("Document", Implementation::Artifact),
            Self::Image => ("Image", Implementation::Artifact),
            Self::Agent => ("Agent", Implementation::Artifact),
            Self::Voice => ("Voice", Implementation::Artifact),
        };
        OutputDefinition {
            id: self.as_str(),
            label,
            implemented: implementation != Implementation::Deferred,
            workspace: if self == Self::Chat {
                "conversation"
            } else {
                "canvas"
            },
            implementation,
        }
    }

    pub fn require_implementation(value: &str, expected: Implementation) -> Result<(), String> {
        let kind = Self::parse_selection(value)?.ok_or("Choose an output before running it.")?;
        if kind.definition().implementation != expected {
            return Err("This output does not support that action.".into());
        }
        Ok(())
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Website => "website",
            Self::Application => "application",
            Self::Presentation => "presentation",
            Self::Document => "document",
            Self::Image => "image",
            Self::Agent => "agent",
            Self::Voice => "voice",
        }
    }

    pub fn parse_selection(value: &str) -> Result<Option<Self>, String> {
        Ok(match value {
            "auto" => None,
            "chat" => Some(Self::Chat),
            "website" => Some(Self::Website),
            "application" => Some(Self::Application),
            "presentation" => Some(Self::Presentation),
            "document" => Some(Self::Document),
            "image" => Some(Self::Image),
            "agent" => Some(Self::Agent),
            "voice" => Some(Self::Voice),
            _ => return Err("Choose a supported output type.".into()),
        })
    }

    pub fn infer(prompt: &str) -> Self {
        let text = prompt.to_lowercase();
        let words = text
            .split(|c: char| !c.is_alphanumeric())
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>();
        if words.first().is_some_and(|word| {
            matches!(*word, "what" | "why" | "how" | "when" | "where" | "should")
        }) {
            return Self::Chat;
        }
        let request_verbs = [
            "build",
            "create",
            "make",
            "design",
            "generate",
            "draft",
            "write",
            "produce",
            "draw",
            "illustrate",
            "want",
            "need",
        ];
        let Some(request_at) = words.iter().position(|word| request_verbs.contains(word)) else {
            return Self::Chat;
        };
        if words[..request_at]
            .windows(2)
            .any(|pair| pair == ["how", "to"])
        {
            return Self::Chat;
        }
        if matches!(words[request_at], "want" | "need")
            && words[request_at + 1..].iter().any(|word| {
                matches!(
                    *word,
                    "know" | "understand" | "learn" | "decide" | "discuss"
                )
            })
        {
            return Self::Chat;
        }
        // The requested artifact precedes its subject in ordinary prompts:
        // "write a report about a website" asks for a document.
        for (index, word) in words.iter().enumerate().skip(request_at + 1) {
            let kind = match *word {
                "website" | "webpage" | "site" if words.get(index + 1) != Some(&"plan") => {
                    Some(Self::Website)
                }
                "landing" if words.get(index + 1) == Some(&"page") => Some(Self::Website),
                "application" | "app" | "software" => Some(Self::Application),
                "presentation" | "slides" | "slideshow" | "deck" => Some(Self::Presentation),
                "document" | "memo" | "report" | "letter" | "proposal" | "brief" => {
                    Some(Self::Document)
                }
                "image" | "picture" | "illustration" | "graphic" | "logo" => Some(Self::Image),
                "agent" | "automation" => Some(Self::Agent),
                "voice" | "audio" | "speech" | "narration" => Some(Self::Voice),
                _ => None,
            };
            if let Some(kind) = kind {
                return kind;
            }
        }
        Self::Chat
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deferred_outputs_cannot_execute_chat_or_website_actions() {
        for kind in OutputType::ALL {
            let definition = kind.definition();
            if !definition.implemented {
                assert_eq!(definition.workspace, "canvas");
                assert!(
                    OutputType::require_implementation(kind.as_str(), Implementation::Website)
                        .is_err()
                );
                assert!(
                    OutputType::require_implementation(kind.as_str(), Implementation::Chat)
                        .is_err()
                );
            }
        }
    }

    #[test]
    fn conservative_auto_routing() {
        let cases = [
            ("Build a simple website for Laundros", OutputType::Website),
            ("I need a simple website for Laundros", OutputType::Website),
            ("Make a landing page for Laundros", OutputType::Website),
            ("Could you create a mobile app?", OutputType::Application),
            (
                "Create a presentation on financing",
                OutputType::Presentation,
            ),
            ("Draft a proposal about our website", OutputType::Document),
            ("I want a brief about the app", OutputType::Document),
            ("Write a report comparing two apps", OutputType::Document),
            ("Draw an image of the city", OutputType::Image),
            ("Generate a logo for my site", OutputType::Image),
            ("Create an agent for customer support", OutputType::Agent),
            ("Produce audio narration for the deck", OutputType::Voice),
            ("What makes a good website?", OutputType::Chat),
            ("Can you review this site?", OutputType::Chat),
            ("What should I write in a proposal?", OutputType::Chat),
            ("Should I build a website?", OutputType::Chat),
            ("Tell me how to build a website", OutputType::Chat),
            ("I want to know how to build a website", OutputType::Chat),
            ("Create a website about how to cook", OutputType::Website),
            ("Create a site plan for the property", OutputType::Chat),
        ];
        for (prompt, expected) in cases {
            assert_eq!(OutputType::infer(prompt), expected, "{prompt}");
        }
    }
}
