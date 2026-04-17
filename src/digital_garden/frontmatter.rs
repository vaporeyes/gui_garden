use serde_yaml;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Frontmatter is the YAML metadata at the top of a note
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Frontmatter {
    /// Title of the note
    pub title: Option<String>,

    /// Custom slug for the note URL
    pub slug: Option<String>,

    /// Whether the note is a draft
    #[serde(default)]
    pub draft: bool,

    /// When the note was created
    pub created: Option<DateTime<Utc>>,

    /// When the note was last updated
    pub updated: Option<DateTime<Utc>>,

    /// Tags associated with the note
    #[serde(default)]
    pub tags: Vec<String>,

    /// Other custom frontmatter fields
    #[serde(flatten)]
    pub custom: HashMap<String, serde_json::Value>,
}

/// Parse frontmatter from a string
pub fn parse_frontmatter(content: &str) -> Option<(Frontmatter, String)> {
    if content.starts_with("---") {
        let content_after_start = &content[3..];

        if let Some(end_pos) = content_after_start.find("---") {
            let yaml_str = &content_after_start[..end_pos].trim();
            let remaining_content = &content_after_start[end_pos + 3..].trim_start();

            match serde_yaml::from_str::<Frontmatter>(yaml_str) {
                Ok(frontmatter) => return Some((frontmatter, remaining_content.to_string())),
                Err(err) => {
                    eprintln!("Error parsing frontmatter: {}", err);
                    return Some((Frontmatter::default(), content.to_string()));
                }
            }
        }
    }

    None
}

/// Generate frontmatter YAML from a Frontmatter struct
pub fn generate_frontmatter(frontmatter: &Frontmatter) -> Result<String, serde_yaml::Error> {
    let yaml = serde_yaml::to_string(frontmatter)?;
    Ok(format!("---\n{}---\n", yaml))
}
