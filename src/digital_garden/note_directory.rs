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

        // Compute backlinks
        compute_backlinks(&mut notes);

        Ok(Self {
            root_path,
            notes,
            folder_structure: Arc::new(folder_structure),
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

/// Compute backlinks for all notes
fn compute_backlinks(notes: &mut HashMap<String, Arc<Note>>) {
    // Collect all links
    let mut backlinks: HashMap<String, HashSet<String>> = HashMap::new();

    for (source_id, note) in notes.iter() {
        for link in &note.links {
            backlinks
                .entry(link.target_id.clone())
                .or_default()
                .insert(source_id.clone());
        }
    }

    // Update notes with backlinks
    for (target_id, source_ids) in backlinks {
        if let Some(note) = notes.get_mut(&target_id) {
            // We need to clone and get a mutable version to update backlinks
            let mut note_clone = (**note).clone();
            note_clone.backlinks = source_ids.into_iter().collect();
            *note = Arc::new(note_clone);
        }
    }
}
