//! Output intent routing. This stays separate from model and tool selection.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Implementation {
    Chat,
    Website,
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
            Self::Application => ("Application", Implementation::Deferred),
            Self::Presentation => ("Presentation", Implementation::Deferred),
            Self::Document => ("Document", Implementation::Deferred),
            Self::Image => ("Image", Implementation::Deferred),
            Self::Agent => ("Agent", Implementation::Deferred),
            Self::Voice => ("Voice", Implementation::Deferred),
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
        let has = |needle: &str| words.contains(&needle);
        let request = [
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
        ]
        .iter()
        .any(|verb| has(verb));
        if !request {
            return Self::Chat;
        }
        if ["website", "webpage", "site", "landing"]
            .iter()
            .any(|word| has(word))
        {
            return Self::Website;
        }
        if ["application", "app", "software"]
            .iter()
            .any(|word| has(word))
        {
            return Self::Application;
        }
        if ["presentation", "slides", "slideshow", "deck"]
            .iter()
            .any(|word| has(word))
        {
            return Self::Presentation;
        }
        if ["document", "memo", "report", "letter", "proposal"]
            .iter()
            .any(|word| has(word))
        {
            return Self::Document;
        }
        if ["image", "picture", "illustration", "graphic", "logo"]
            .iter()
            .any(|word| has(word))
        {
            return Self::Image;
        }
        if has("agent") || has("automation") {
            return Self::Agent;
        }
        if ["voice", "audio", "speech", "narration"]
            .iter()
            .any(|word| has(word))
        {
            return Self::Voice;
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
        assert_eq!(
            OutputType::infer("Build a simple website for Laundros"),
            OutputType::Website
        );
        assert_eq!(
            OutputType::infer("What makes a good website?"),
            OutputType::Chat
        );
        assert_eq!(
            OutputType::infer("Create a presentation on financing"),
            OutputType::Presentation
        );
        assert_eq!(
            OutputType::infer("Draw an image of the city"),
            OutputType::Image
        );
        assert_eq!(
            OutputType::infer("Generate an image of the city"),
            OutputType::Image
        );
    }
}
