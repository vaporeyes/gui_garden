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
        let accent = ui.visuals().selection.stroke.color;

        // Quick-filter search bar
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("🔍").weak());
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.search_query)
                    .desired_width(f32::INFINITY)
                    .hint_text("filter notes"),
            );
            if resp.changed() {
                if !self.search_query.is_empty() {
                    self.search_results = note_directory.search(&self.search_query);
                } else {
                    self.search_results.clear();
                }
            }
        });

        if !self.search_query.is_empty() {
            if ui.small_button("clear filter").clicked() {
                self.search_query.clear();
                self.search_results.clear();
            }
        }

        ui.add_space(10.0);
        ui.checkbox(&mut self.show_graph, "Show graph view");
        ui.add_space(10.0);
        ui.separator();

        // Show search results, or the folder tree
        if !self.search_results.is_empty() {
            section_label(ui, "matches", accent);
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for note in &self.search_results {
                        let is_selected = self.selected_note.as_deref() == Some(&note.id);
                        let text = if is_selected {
                            RichText::new(note.title()).strong().color(accent)
                        } else {
                            RichText::new(note.title())
                        };

                        if ui.link(text).clicked() {
                            self.selected_note = Some(note.id.clone());
                            clicked_note = Some(note.id.clone());
                        }
                    }
                });
        } else {
            section_label(ui, "notes", accent);
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if let Some(click) =
                        self.show_folder(ui, &note_directory.folder_structure, note_directory, 0)
                    {
                        clicked_note = Some(click);
                    }
                });
        }

        clicked_note
    }

    /// Show a folder in the sidebar using a native `CollapsingHeader`.
    fn show_folder(
        &mut self,
        ui: &mut Ui,
        folder: &Arc<Folder>,
        note_directory: &NoteDirectory,
        depth: usize,
    ) -> Option<String> {
        let mut clicked_note = None;
        let folder_id = folder.path.to_string_lossy().to_string();

        // Top-level "posts" folder is expanded by default; nested folders
        // start collapsed to keep the tree scannable.
        let header =
            egui::CollapsingHeader::new(RichText::new(&folder.name).strong().monospace())
                .id_salt(&folder_id)
                .default_open(depth == 0);

        header.show(ui, |ui| {
            for subfolder in &folder.folders {
                if let Some(note_id) = self.show_folder(ui, subfolder, note_directory, depth + 1) {
                    clicked_note = Some(note_id);
                }
            }

            for note_id in &folder.notes {
                let Some(note) = note_directory.get_note(note_id) else { continue };
                if note.is_draft() {
                    continue;
                }

                let is_selected = self.selected_note.as_deref() == Some(note_id);
                let accent = ui.visuals().selection.stroke.color;
                let text = if is_selected {
                    RichText::new(note.title()).strong().color(accent)
                } else {
                    RichText::new(note.title())
                };

                if ui
                    .add(egui::Label::new(text).sense(egui::Sense::click()))
                    .clicked()
                {
                    self.selected_note = Some(note_id.clone());
                    clicked_note = Some(note_id.clone());
                }
            }
        });

        clicked_note
    }
}

/// Small-caps section label, colored with the theme accent.
fn section_label(ui: &mut Ui, text: &str, accent: egui::Color32) {
    ui.add_space(4.0);
    ui.label(
        RichText::new(text.to_uppercase())
            .small()
            .strong()
            .color(accent),
    );
    ui.add_space(4.0);
}
