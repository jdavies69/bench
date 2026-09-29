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
    if let Some(name) = explicit_name(text) {
        if let Some(id) = db::project_by_name(conn, &name)? {
            return Ok(id);
        }
    }
    let mut matches = Vec::new();
    for project in projects {
        let name = project.name.to_lowercase();
        if name.len() >= 4 && contains_phrase(&lower, &name) {
            matches.push(project.id);
        }
    }
    if matches.len() == 1 {
        return Ok(matches.remove(0));
    }
    if matches.len() > 1 {
        return Ok(MISC_ID.to_owned());
    }
    if let Some(name) = explicit_name(text) {
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

fn explicit_name_line(text: &str) -> Option<String> {
    let beginning = text.split_whitespace().next()?.to_ascii_lowercase();
    if !matches!(
        beginning.as_str(),
        "build" | "create" | "make" | "design" | "draft" | "write" | "update"
    ) {
        return None;
    }
    let words: Vec<&str> = text.split_whitespace().collect();
    for (i, word) in words.iter().enumerate() {
        if !word.eq_ignore_ascii_case("for") {
            continue;
        }
        let mut name = Vec::new();
        for candidate in words.iter().skip(i + 1).take(3) {
            let clean = candidate.trim_matches(|c: char| !c.is_alphanumeric());
            if clean.is_empty() || matches!(clean, "I" | "A" | "The" | "My" | "Our") {
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
        if !name.is_empty() {
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
    fn ambiguous_existing_matches_fall_back_to_miscellaneous() {
        let conn = Connection::open_in_memory().unwrap();
        db::initialize(&conn).unwrap();
        db::create_project(&conn, "Laundros").unwrap();
        db::create_project(&conn, "Sunday Capital").unwrap();
        assert_eq!(
            classify(&conn, "Compare Laundros and Sunday Capital").unwrap(),
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
}
