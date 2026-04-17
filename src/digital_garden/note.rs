use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
    #[serde(default, alias = "pubDate", deserialize_with = "deserialize_flexible_date")]
    pub created: Option<DateTime<Utc>>,

    /// When the note was last updated
    #[serde(default, deserialize_with = "deserialize_flexible_date")]
    pub updated: Option<DateTime<Utc>>,

    /// Tags associated with the note
    #[serde(default)]
    pub tags: Vec<String>,

    /// Other custom frontmatter fields
    #[serde(flatten)]
    pub custom: HashMap<String, serde_json::Value>,
}

/// A link between notes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct NoteLink {
    /// The target note ID
    pub target_id: String,

    /// The text to display for the link
    pub display_text: String,

    /// Whether this is an embed (!) link
    pub is_embed: bool,
}

/// Represents a note in the digital garden
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    /// Unique identifier for the note (typically the filename without extension)
    pub id: String,

    /// Path to the note file, relative to the notes directory
    pub path: PathBuf,

    /// Frontmatter metadata
    pub frontmatter: Frontmatter,

    /// Raw content of the note (Markdown)
    pub content: String,

    /// Links within this note to other notes
    pub links: Vec<NoteLink>,

    /// Backlinks - notes that link to this note
    #[serde(skip)]
    pub backlinks: Vec<String>,
}

impl Note {
    /// Create a new note from a file
    pub fn from_file<P: AsRef<Path>>(path: P, content: String) -> Result<Self, String> {
        let path = path.as_ref();

        // Extract the note ID from the filename
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| format!("Invalid filename: {:?}", path))?
            .to_string();

        // Parse frontmatter and content
        let (frontmatter, content) =
            parse_frontmatter(&content).unwrap_or((Frontmatter::default(), content));

        // Extract links from content
        let links = extract_links(&content);

        Ok(Self {
            id,
            path: path.to_path_buf(),
            frontmatter,
            content,
            links,
            backlinks: Vec::new(),
        })
    }

    /// Get the title of the note, falling back to ID if not specified
    pub fn title(&self) -> String {
        self.frontmatter.title.clone().unwrap_or_else(|| {
            // Convert ID to title case
            let mut title = String::new();
            let id = self.id.replace('_', " ").replace('-', " ");

            let mut capitalize = true;
            for c in id.chars() {
                if c.is_alphanumeric() {
                    if capitalize {
                        title.extend(c.to_uppercase());
                        capitalize = false;
                    } else {
                        title.push(c);
                    }
                } else {
                    title.push(c);
                    capitalize = true;
                }
            }

            title
        })
    }

    /// Get the slug for the note's URL
    pub fn slug(&self) -> String {
        self.frontmatter
            .slug
            .clone()
            .unwrap_or_else(|| self.id.clone())
    }

    /// Check if the note is a draft
    pub fn is_draft(&self) -> bool {
        self.frontmatter.draft
    }
}

/// Accepts either an RFC3339 datetime (`2026-03-23T10:00:00Z`) or a plain YAML
/// date (`2026-03-23`, as Astro uses in `pubDate`). Plain dates are promoted
/// to midnight UTC.
fn deserialize_flexible_date<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        DateTime(DateTime<Utc>),
        Naive(chrono::NaiveDate),
    }

    Ok(Option::<Raw>::deserialize(deserializer)?.map(|r| match r {
        Raw::DateTime(dt) => dt,
        Raw::Naive(d) => d.and_hms_opt(0, 0, 0).unwrap().and_utc(),
    }))
}

/// Parses frontmatter from content, returning (frontmatter, remaining_content)
fn parse_frontmatter(content: &str) -> Option<(Frontmatter, String)> {
    if content.starts_with("---\n") || content.starts_with("---\r\n") {
        let end_delimiter = if content.starts_with("---\n") {
            "---\n"
        } else {
            "---\r\n"
        };
        let content_after_start = &content[end_delimiter.len()..];

        if let Some(end_pos) = content_after_start.find(end_delimiter) {
            let yaml_str = &content_after_start[..end_pos];
            let content_str = &content_after_start[end_pos + end_delimiter.len()..];

            if let Ok(frontmatter) = serde_yaml::from_str(yaml_str) {
                return Some((frontmatter, content_str.to_string()));
            }
        }
    }

    None
}

/// Extract links from content
fn extract_links(content: &str) -> Vec<NoteLink> {
    let mut links = Vec::new();
    let mut pos = 0;

    while let Some(start) = content[pos..].find("[[") {
        let start = pos + start;
        if let Some(end) = content[start..].find("]]") {
            let end = start + end;
            let link_text = &content[start + 2..end];

            let is_embed = start > 0 && &content[start - 1..start] == "!";
            let display_text;
            let target_id;

            if let Some(pipe_pos) = link_text.find('|') {
                target_id = link_text[..pipe_pos].trim().to_string();
                display_text = link_text[pipe_pos + 1..].trim().to_string();
            } else {
                target_id = link_text.trim().to_string();
                display_text = target_id.clone();
            }

            links.push(NoteLink {
                target_id,
                display_text,
                is_embed,
            });

            pos = end + 2;
        } else {
            break;
        }
    }

    links
}
