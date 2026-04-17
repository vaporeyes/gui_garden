use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::note::Note;

/// Represents a folder in the note directory
#[derive(Debug, Clone)]
pub struct Folder {
    /// Name of the folder
    pub name: String,

    /// Path to the folder
    pub path: PathBuf,

    /// Child folders
    pub folders: Vec<Arc<Folder>>,

    /// Notes in this folder
    pub notes: Vec<String>,
}

/// Manages the collection of notes and their relationships
#[derive(Debug, Clone)]
pub struct NoteDirectory {
    /// Path to the root notes directory
    pub root_path: PathBuf,

    /// All notes, indexed by ID
    pub notes: HashMap<String, Arc<Note>>,

    /// Folder structure
    pub folder_structure: Arc<Folder>,

    /// Map of target note id → source note ids that link to it. Stored here
    /// rather than on `Note` so populating it doesn't require cloning every
    /// note (which was the original bug).
    pub backlinks: HashMap<String, Vec<String>>,
}

impl NoteDirectory {
    /// Create a new note directory from a path
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let root_path = path.as_ref().to_path_buf();

        if !root_path.exists() || !root_path.is_dir() {
            return Err(format!("Invalid directory path: {:?}", root_path));
        }

        let mut notes = HashMap::new();
        let folder_structure = scan_folder(&root_path, &root_path, &mut notes)?;
        let backlinks = compute_backlinks(&notes);

        Ok(Self {
            root_path,
            notes,
            folder_structure: Arc::new(folder_structure),
            backlinks,
        })
    }

    /// Get a note by its ID
    pub fn get_note(&self, id: &str) -> Option<Arc<Note>> {
        self.notes.get(id).cloned()
    }

    /// Get a note by its slug
    pub fn get_note_by_slug(&self, slug: &str) -> Option<Arc<Note>> {
        for note in self.notes.values() {
            if note.slug() == slug {
                return Some(note.clone());
            }
        }
        None
    }

    /// Resolve a wiki-link query to a canonical note. Tries, in order:
    ///   1. Exact id match (`elegy-campaign-player`).
    ///   2. Case-insensitive id match (`Elegy-Campaign-Player`).
    ///   3. Slug match (from frontmatter `slug:`).
    ///   4. Case-insensitive title match (`Elegy Campaign Player`).
    ///
    /// Returns `None` for drafts — published notes shouldn't ever resolve
    /// to a draft at click time.
    pub fn resolve_link(&self, query: &str) -> Option<Arc<Note>> {
        resolve_link_id(query, &self.notes)
            .and_then(|id| self.notes.get(&id).cloned())
            .filter(|n| !n.is_draft())
    }

    /// Ids of notes that link *to* `id`. Empty slice if none.
    pub fn backlinks(&self, id: &str) -> &[String] {
        self.backlinks
            .get(id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get all non-draft notes
    pub fn published_notes(&self) -> Vec<Arc<Note>> {
        self.notes
            .values()
            .filter(|note| !note.is_draft())
            .cloned()
            .collect()
    }

    /// Get all notes with a specific tag
    pub fn notes_with_tag(&self, tag: &str) -> Vec<Arc<Note>> {
        self.notes
            .values()
            .filter(|note| note.frontmatter.tags.contains(&tag.to_string()))
            .cloned()
            .collect()
    }

    /// Search notes by keyword
    pub fn search(&self, query: &str) -> Vec<Arc<Note>> {
        let query = query.to_lowercase();

        self.notes
            .values()
            .filter(|note| {
                if note.is_draft() {
                    return false;
                }

                note.title().to_lowercase().contains(&query)
                    || note.content.to_lowercase().contains(&query)
                    || note
                        .frontmatter
                        .tags
                        .iter()
                        .any(|tag| tag.to_lowercase().contains(&query))
            })
            .cloned()
            .collect()
    }
}

/// Pure variant of `NoteDirectory::resolve_link` that returns just the
/// canonical id. Exposed at module scope so backlink construction can use
/// the same resolution rules without needing a fully-built `NoteDirectory`.
fn resolve_link_id(query: &str, notes: &HashMap<String, Arc<Note>>) -> Option<String> {
    if notes.contains_key(query) {
        return Some(query.to_string());
    }
    let lower = query.to_lowercase();
    if let Some(id) = notes.keys().find(|id| id.to_lowercase() == lower) {
        return Some(id.clone());
    }
    for (id, note) in notes.iter() {
        if note.slug() == query {
            return Some(id.clone());
        }
    }
    for (id, note) in notes.iter() {
        if note.title().to_lowercase() == lower {
            return Some(id.clone());
        }
    }
    None
}

/// Scan a folder and build the folder structure
fn scan_folder(
    root_path: &Path,
    folder_path: &Path,
    notes: &mut HashMap<String, Arc<Note>>,
) -> Result<Folder, String> {
    let folder_name = folder_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("Root")
        .to_string();

    let mut folder = Folder {
        name: folder_name,
        path: folder_path.to_path_buf(),
        folders: Vec::new(),
        notes: Vec::new(),
    };

    let entries = match fs::read_dir(folder_path) {
        Ok(entries) => entries,
        Err(err) => return Err(format!("Failed to read directory: {}", err)),
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                eprintln!("Error reading directory entry: {}", err);
                continue;
            }
        };

        let path = entry.path();

        if path.is_dir() {
            // Recursively process subfolder
            match scan_folder(&root_path, &path, notes) {
                Ok(subfolder) => folder.folders.push(Arc::new(subfolder)),
                Err(err) => eprintln!("Error scanning subfolder {:?}: {}", path, err),
            }
        } else if path.is_file() && path.extension() == Some(OsStr::new("md")) {
            // Process markdown file
            match fs::read_to_string(&path) {
                Ok(content) => {
                    let relative_path = path.strip_prefix(root_path).unwrap_or(&path);

                    match Note::from_file(relative_path, content) {
                        Ok(note) => {
                            let id = note.id.clone();
                            folder.notes.push(id.clone());
                            notes.insert(id, Arc::new(note));
                        }
                        Err(err) => eprintln!("Error parsing note {:?}: {}", path, err),
                    }
                }
                Err(err) => eprintln!("Error reading file {:?}: {}", path, err),
            }
        }
    }

    // Sort folders and notes alphabetically
    folder.folders.sort_by(|a, b| a.name.cmp(&b.name));
    folder.notes.sort();

    Ok(folder)
}

/// Build the `target_id → [source_ids]` map without mutating the notes.
/// Link targets are resolved through `resolve_link_id` so wiki-links written
/// in title or slug form contribute to the right canonical bucket.
fn compute_backlinks(notes: &HashMap<String, Arc<Note>>) -> HashMap<String, Vec<String>> {
    let mut buckets: HashMap<String, HashSet<String>> = HashMap::new();

    for (source_id, note) in notes.iter() {
        for link in &note.links {
            let target = resolve_link_id(&link.target_id, notes)
                .unwrap_or_else(|| link.target_id.clone());
            buckets
                .entry(target)
                .or_default()
                .insert(source_id.clone());
        }
    }

    buckets
        .into_iter()
        .map(|(k, v)| {
            let mut sources: Vec<String> = v.into_iter().collect();
            sources.sort();
            (k, sources)
        })
        .collect()
}

// ---------- tests ----------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::digital_garden::note::{Frontmatter, NoteLink};

    fn mk_note(id: &str, title: Option<&str>, slug: Option<&str>, links: Vec<&str>) -> Arc<Note> {
        Arc::new(Note {
            id: id.to_string(),
            path: PathBuf::from(format!("{}.md", id)),
            frontmatter: Frontmatter {
                title: title.map(|s| s.to_string()),
                slug: slug.map(|s| s.to_string()),
                ..Default::default()
            },
            content: String::new(),
            links: links
                .into_iter()
                .map(|t| NoteLink {
                    target_id: t.to_string(),
                    display_text: t.to_string(),
                    is_embed: false,
                })
                .collect(),
        })
    }

    fn corpus() -> HashMap<String, Arc<Note>> {
        let mut m = HashMap::new();
        m.insert(
            "elegy-campaign-player".to_string(),
            mk_note(
                "elegy-campaign-player",
                Some("Elegy Campaign Player"),
                Some("elegy"),
                vec![],
            ),
        );
        m.insert(
            "intro".to_string(),
            mk_note(
                "intro",
                Some("Intro"),
                None,
                vec!["elegy-campaign-player", "Elegy Campaign Player", "MISSING"],
            ),
        );
        m.insert(
            "sidenote".to_string(),
            mk_note("sidenote", None, None, vec!["elegy"]),
        );
        m
    }

    #[test]
    fn resolve_link_exact_id() {
        let c = corpus();
        assert_eq!(
            resolve_link_id("elegy-campaign-player", &c).as_deref(),
            Some("elegy-campaign-player")
        );
    }

    #[test]
    fn resolve_link_case_insensitive_id() {
        let c = corpus();
        assert_eq!(
            resolve_link_id("Elegy-Campaign-Player", &c).as_deref(),
            Some("elegy-campaign-player")
        );
    }

    #[test]
    fn resolve_link_by_slug() {
        let c = corpus();
        assert_eq!(
            resolve_link_id("elegy", &c).as_deref(),
            Some("elegy-campaign-player")
        );
    }

    #[test]
    fn resolve_link_by_title_case_insensitive() {
        let c = corpus();
        assert_eq!(
            resolve_link_id("elegy campaign player", &c).as_deref(),
            Some("elegy-campaign-player")
        );
        assert_eq!(
            resolve_link_id("ELEGY CAMPAIGN PLAYER", &c).as_deref(),
            Some("elegy-campaign-player")
        );
    }

    #[test]
    fn resolve_link_returns_none_for_missing() {
        let c = corpus();
        assert!(resolve_link_id("does-not-exist", &c).is_none());
    }

    #[test]
    fn backlinks_use_canonical_ids_across_link_styles() {
        let c = corpus();
        let bl = compute_backlinks(&c);

        // Both "elegy-campaign-player" (exact id) and "Elegy Campaign Player"
        // (title form) in `intro`, plus "elegy" (slug) in `sidenote`, should
        // all land on the canonical id's bucket.
        let sources = bl.get("elegy-campaign-player").expect("should have backlinks");
        assert!(sources.contains(&"intro".to_string()));
        assert!(sources.contains(&"sidenote".to_string()));
        // Dedupe — intro shouldn't appear twice just because it used two link forms.
        assert_eq!(
            sources.iter().filter(|s| s == &"intro").count(),
            1,
            "intro should appear exactly once in backlinks"
        );
    }

    #[test]
    fn backlinks_unresolved_links_still_bucket_under_raw_target() {
        let c = corpus();
        let bl = compute_backlinks(&c);
        assert!(
            bl.get("MISSING").is_some(),
            "dead wiki-links still show up so we can render a dead-link affordance"
        );
    }
}
