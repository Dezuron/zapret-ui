//! One-time "What's new" notes for the current application version.
//!
//! The changelog is embedded at compile time, so the window works offline and
//! always shows only the section for the running version.

use serde::Serialize;

const CHANGELOG: &str = include_str!("../../CHANGELOG.md");

#[derive(Debug, Clone, Serialize)]
pub struct WhatsNew {
    pub version: String,
    pub notes: String,
    pub url: String,
}

pub fn current() -> Option<WhatsNew> {
    let version = env!("CARGO_PKG_VERSION").to_string();
    let notes = extract_section(CHANGELOG, &version)?;
    Some(WhatsNew {
        url: format!("https://github.com/Dezuron/zapret-ui/releases/tag/v{version}"),
        version,
        notes,
    })
}

/// Extracts the body of `## [<version>]` up to the next `## [` heading.
fn extract_section(text: &str, version: &str) -> Option<String> {
    let marker = format!("## [{version}]");
    let mut lines = text.lines();
    let mut found = false;
    let mut body = Vec::new();

    for line in lines.by_ref() {
        if line.starts_with(&marker) {
            found = true;
            break;
        }
    }
    if !found {
        return None;
    }

    for line in lines {
        if line.starts_with("## [") {
            break;
        }
        body.push(line);
    }

    let body = body.join("\n").trim().to_string();
    if body.is_empty() {
        None
    } else {
        Some(body)
    }
}

#[cfg(test)]
mod tests {
    use super::extract_section;

    const SAMPLE: &str = r#"# Changelog

## [2.0.0] — 2026-01-02

### Добавлено

- Новое окно.

## [1.0.0] — 2026-01-01

- Старое.
"#;

    #[test]
    fn extracts_only_the_current_version() {
        let notes = extract_section(SAMPLE, "2.0.0").unwrap();
        assert!(notes.contains("Новое окно"));
        assert!(!notes.contains("Старое"));
        assert!(!notes.contains("## [1.0.0]"));
    }

    #[test]
    fn missing_version_returns_none() {
        assert!(extract_section(SAMPLE, "9.9.9").is_none());
    }

    #[test]
    fn embedded_changelog_has_the_current_version() {
        assert!(super::current().is_some());
    }
}
