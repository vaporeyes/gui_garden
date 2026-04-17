use super::note::Note;
use super::note_directory::{Folder, NoteDirectory};
use egui::{Color32, RichText, Ui};
use std::collections::HashMap;
use std::sync::Arc;

/// State for the sidebar
pub struct Sidebar {
    /// Currently selected note ID
    pub selected_note: Option<String>,

    /// Expanded folders
    pub expanded_folders: HashMap<String, bool>,

    /// Search query
    pub search_query: String,

    /// Search results
    pub search_results: Vec<Arc<Note>>,

    /// Show graph view
    pub show_graph: bool,
}

impl Default for Sidebar {
    fn default() -> Self {
        Self {
            selected_note: None,
            expanded_folders: HashMap::new(),
            search_query: String::new(),
            search_results: Vec::new(),
            show_graph: true,
        }
    }
}

impl Sidebar {
    /// Create a new sidebar
    pub fn new() -> Self {
        Self::default()
    }

    /// Show the sidebar
    pub fn ui(&mut self, ui: &mut Ui, note_directory: &NoteDirectory) -> Option<String> {
        let mut clicked_note = None;

        // Search bar
        ui.horizontal(|ui| {
            ui.label("🔍");
            if ui.text_edit_singleline(&mut self.search_query).changed() {
                if !self.search_query.is_empty() {
                    self.search_results = note_directory.search(&self.search_query);
                } else {
                    self.search_results.clear();
                }
            }

            if ui.button("✕").clicked() {
                self.search_query.clear();
                self.search_results.clear();
            }
        });

        ui.separator();

        // Graph view toggle
        if ui
            .checkbox(&mut self.show_graph, "Show Graph View")
            .clicked()
        {
            // Toggle was handled by the checkbox
        }

        ui.separator();

        // Show search results if any
        if !self.search_results.is_empty() {
            ui.heading("Search Results");
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                for note in &self.search_results {
                    let is_selected = self.selected_note.as_deref() == Some(&note.id);
                    let text = if is_selected {
                        RichText::new(&note.title())
                            .strong()
                            .color(ui.visuals().selection.stroke.color)
                    } else {
                        RichText::new(&note.title())
                    };

                    if ui.link(text).clicked() {
                        self.selected_note = Some(note.id.clone());
                        clicked_note = Some(note.id.clone());
                    }
                }
            });
        } else {
            // Show folder structure
            ui.heading("Notes");
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                if let Some(click) =
                    self.show_folder(ui, &note_directory.folder_structure, note_directory, 0)
                {
                    clicked_note = Some(click);
                }
            });
        }

        clicked_note
    }

    /// Show a folder in the sidebar
    fn show_folder(
        &mut self,
        ui: &mut Ui,
        folder: &Arc<Folder>,
        note_directory: &NoteDirectory,
        depth: usize,
    ) -> Option<String> {
        let indent = "    ".repeat(depth);
        let mut clicked_note = None;

        // Create a unique ID for the folder based on its path
        let folder_id = folder.path.to_string_lossy().to_string();
        let is_expanded = *self
            .expanded_folders
            .entry(folder_id.clone())
            .or_insert(depth == 0);

        // Folder header with expand/collapse icon
        let folder_icon = if is_expanded { "📂" } else { "📁" };
        let folder_text = format!("{}{} {}", indent, folder_icon, folder.name);

        if ui.button(folder_text).clicked() {
            *self.expanded_folders.get_mut(&folder_id).unwrap() = !is_expanded;
        }

        // Show folder contents if expanded
        if is_expanded {
            // Show subfolders
            for subfolder in &folder.folders {
                if let Some(note_id) = self.show_folder(ui, subfolder, note_directory, depth + 1) {
                    clicked_note = Some(note_id);
                }
            }

            // Show notes
            for note_id in &folder.notes {
                if let Some(note) = note_directory.get_note(note_id) {
                    // Skip draft notes
                    if note.is_draft() {
                        continue;
                    }

                    let is_selected = self.selected_note.as_deref() == Some(note_id);
                    let note_icon = "📄";
                    let note_text = format!("{}{} {}", indent, note_icon, note.title());

                    let text = if is_selected {
                        RichText::new(note_text)
                            .strong()
                            .color(ui.visuals().selection.stroke.color)
                    } else {
                        RichText::new(note_text)
                    };

                    ui.horizontal(|ui| {
                        ui.add_space((depth + 1) as f32 * 10.0);

                        if ui.link(text).clicked() {
                            self.selected_note = Some(note_id.clone());
                            clicked_note = Some(note_id.clone());
                        }
                    });
                }
            }
        }

        clicked_note
    }
}
