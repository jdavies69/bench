use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::db::{self, MISC_ID};

// Deliberately conservative: a weak guess is less useful than Miscellaneous.
pub fn classify(conn: &Connection, text: &str) -> Result<String, String> {
    let lower = text.to_lowercase();
    let projects = db::projects(conn)?
        .into_iter()
        .filter(|p| !p.is_system)
        .collect::<Vec<_>>();
    let mut matches = Vec::new();
    for project in &projects {
        let name = project.name.to_lowercase();
        if name.len() >= 4 && contains_phrase(&lower, &name) {
            matches.push(project);
        }
    }
    if matches.len() > 1 {
        return Ok(MISC_ID.to_owned());
    }
    let matched_names = matches
        .iter()
        .map(|project| project.name.as_str())
        .collect::<Vec<_>>();
    if ambiguous_for_names(text, &matched_names) {
        return Ok(MISC_ID.to_owned());
    }
    if matches.len() == 1 {
        return Ok(matches.remove(0).id.clone());
    }
    if let Some(name) = explicit_name(text) {
        if let Some(id) = db::project_by_name(conn, &name)? {
            return Ok(id);
        }
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO projects(id, name) VALUES (?1, ?2)",
            params![id, name],
        )
        .map_err(|e| e.to_string())?;
        return Ok(id);
    }
    Ok(MISC_ID.to_owned())
}

fn contains_phrase(haystack: &str, needle: &str) -> bool {
    haystack.match_indices(needle).any(|(start, _)| {
        let end = start + needle.len();
        let before = haystack[..start].chars().next_back();
        let after = haystack[end..].chars().next();
        !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
    })
}

fn explicit_name(text: &str) -> Option<String> {
    text.lines().find_map(explicit_name_line)
}

fn ambiguous_for_names(text: &str, matched_names: &[&str]) -> bool {
    text.lines().any(|line| {
        let words: Vec<&str> = line.split_whitespace().collect();
        words.windows(3).any(|triple| {
            let next = triple[2].trim_matches(|c: char| !c.is_alphanumeric());
            if !triple[1].eq_ignore_ascii_case("and")
                || !next.chars().next().is_some_and(char::is_uppercase)
            {
                return false;
            }
            let previous = triple[0].trim_matches(|c: char| !c.is_alphanumeric());
            let connector = format!("{previous} and {next}").to_lowercase();
            !matched_names
                .iter()
                .any(|name| name.to_lowercase().contains(&connector))
        })
    })
}

fn explicit_name_line(text: &str) -> Option<String> {
    let beginning = text.split_whitespace().next()?.to_ascii_lowercase();
    if !matches!(
        beginning.as_str(),
        "build" | "create" | "make" | "design" | "draft" | "write" | "update"
    ) {
        return None;
    }
    let words: Vec<&str> = text.split_whitespace().collect();
    // A weak document request needs more evidence than a capitalized recipient.
    let strong_artifact = words.iter().enumerate().any(|(index, word)| {
        if word.eq_ignore_ascii_case("site")
            && words
                .get(index + 1)
                .is_some_and(|next| next.eq_ignore_ascii_case("plan"))
        {
            return false;
        }
        matches!(
            word.to_ascii_lowercase()
                .trim_matches(|c: char| !c.is_alphanumeric()),
            "website"
                | "webpage"
                | "site"
                | "app"
                | "application"
                | "project"
                | "logo"
                | "presentation"
                | "deck"
        )
    });
    let document_artifact = words.iter().any(|word| {
        matches!(
            word.to_ascii_lowercase()
                .trim_matches(|c: char| !c.is_alphanumeric()),
            "note" | "memo" | "report" | "proposal" | "letter" | "document"
        )
    });
    if !strong_artifact && !document_artifact {
        return None;
    }
    for (i, word) in words.iter().enumerate() {
        if !word.eq_ignore_ascii_case("for") {
            continue;
        }
        if ambiguous_for_names(text, &[]) {
            return None;
        }
        let mut name = Vec::new();
        for candidate in words.iter().skip(i + 1).take(3) {
            let clean = candidate.trim_matches(|c: char| !c.is_alphanumeric());
            if clean.is_empty() || matches!(clean, "I" | "A" | "The" | "My" | "Our") {
                break;
            }
            if matches!(
                clean.to_ascii_lowercase().as_str(),
                "monday"
                    | "tuesday"
                    | "wednesday"
                    | "thursday"
                    | "friday"
                    | "saturday"
                    | "sunday"
                    | "today"
                    | "tomorrow"
            ) && !words
                .get(i + 2)
                .is_some_and(|next| next.chars().next().is_some_and(char::is_uppercase))
            {
                break;
            }
            if clean.chars().next().is_some_and(char::is_uppercase)
                && clean
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == '&')
            {
                name.push(clean);
            } else {
                break;
            }
        }
        if !name.is_empty() && (strong_artifact || (document_artifact && name.len() >= 2)) {
            let value = name.join(" ");
            if value.chars().count() <= 48 {
                return Some(value);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_reuses_explicit_project() {
        let conn = Connection::open_in_memory().unwrap();
        db::initialize(&conn).unwrap();
        let id = classify(&conn, "Make a site for Sunday Capital").unwrap();
        assert_ne!(id, MISC_ID);
        assert_eq!(
            id,
            classify(&conn, "Update the Sunday Capital logo").unwrap()
        );
        assert_eq!(classify(&conn, "Summarize this note").unwrap(), MISC_ID);
    }

    #[test]
    fn multiword_organization_in_a_document_request_can_create_project() {
        let conn = Connection::open_in_memory().unwrap();
        db::initialize(&conn).unwrap();
        let id = classify(&conn, "Draft a note for Sunday Capital").unwrap();
        assert_ne!(id, MISC_ID);
        assert_eq!(
            db::project_by_name(&conn, "Sunday Capital").unwrap(),
            Some(id)
        );
        assert_eq!(
            classify(&conn, "Draft a letter for Sarah").unwrap(),
            MISC_ID
        );
    }

    #[test]
    fn ambiguous_existing_matches_fall_back_to_miscellaneous() {
        let conn = Connection::open_in_memory().unwrap();
        db::initialize(&conn).unwrap();
        db::create_project(&conn, "Laundros").unwrap();
        db::create_project(&conn, "Sunday Capital").unwrap();
        for prompt in [
            "Compare Laundros and Sunday Capital",
            "Build a website for Laundros comparing Sunday Capital",
            "Create a deck for Sunday Capital about Laundros",
        ] {
            assert_eq!(classify(&conn, prompt).unwrap(), MISC_ID, "{prompt}");
        }
    }

    #[test]
    fn and_inside_an_existing_project_name_is_not_ambiguity() {
        let conn = Connection::open_in_memory().unwrap();
        db::initialize(&conn).unwrap();
        let smith = db::create_project(&conn, "Smith and Sons").unwrap();
        assert_eq!(
            classify(&conn, "Build a website for Smith and Sons").unwrap(),
            smith.id
        );

        db::create_project(&conn, "Laundros").unwrap();
        assert_eq!(
            classify(&conn, "Build a website for Smith and Sons and Laundros").unwrap(),
            MISC_ID
        );
    }

    #[test]
    fn later_context_can_create_an_explicit_project() {
        let conn = Connection::open_in_memory().unwrap();
        db::initialize(&conn).unwrap();
        let id = classify(&conn, "Help me plan this\nBuild a website for Laundros").unwrap();
        assert_ne!(id, MISC_ID);
        assert_eq!(db::project_by_name(&conn, "Laundros").unwrap(), Some(id));
    }

    #[test]
    fn realistic_existing_matches_and_weak_new_names() {
        let conn = Connection::open_in_memory().unwrap();
        db::initialize(&conn).unwrap();
        let laundros = db::create_project(&conn, "Laundros").unwrap();
        for prompt in [
            "Please update the Laundros header",
            "Can we discuss Laundros pricing?",
            "Design a logo for Laundros",
        ] {
            assert_eq!(classify(&conn, prompt).unwrap(), laundros.id, "{prompt}");
        }
        for prompt in [
            "Write a note for Monday",
            "Draft a letter for Sarah",
            "Build a site plan for Monday",
            "Build a website for Monday",
            "Build a website for Laundros and Sunday Capital",
            "Summarize the meeting",
        ] {
            assert_eq!(classify(&conn, prompt).unwrap(), MISC_ID, "{prompt}");
        }
        assert!(db::project_by_name(&conn, "Monday").unwrap().is_none());
        assert!(db::project_by_name(&conn, "Sarah").unwrap().is_none());
    }
}
