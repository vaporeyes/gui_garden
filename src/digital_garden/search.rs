use super::note::Note;
use super::note_directory::NoteDirectory;
use egui::{Color32, RichText, Ui};
use std::sync::Arc;

/// A search result with context
pub struct SearchResult {
    /// The note containing the match
    pub note: Arc<Note>,

    /// The matching snippet
    pub snippet: String,

    /// Position of the match in the original content
    pub position: usize,
}

/// Search manager
pub struct Search {
    /// Current search query
    pub query: String,

    /// Recent search results
    pub results: Vec<SearchResult>,

    /// Currently selected result index
    pub selected_index: Option<usize>,
}

impl Default for Search {
    fn default() -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            selected_index: None,
        }
    }
}

impl Search {
    /// Create a new search manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Perform a search
    pub fn search(&mut self, note_directory: &NoteDirectory) {
        if self.query.is_empty() {
            self.results.clear();
            self.selected_index = None;
            return;
        }

        let query = self.query.to_lowercase();
        let mut results = Vec::new();

        for note in note_directory.published_notes() {
            // Search in title
            if note.title().to_lowercase().contains(&query) {
                results.push(SearchResult {
                    note: note.clone(),
                    snippet: format!("Title: {}", note.title()),
                    position: 0, // Position in title
                });
            }

            // Search in content
            let mut content_pos = 0;
            while let Some(pos) = note.content[content_pos..].to_lowercase().find(&query) {
                let real_pos = content_pos + pos;

                // Get context around the match
                let snippet_start = real_pos.saturating_sub(50);
                let snippet_end = (real_pos + query.len() + 50).min(note.content.len());
                let mut snippet = note.content[snippet_start..snippet_end].to_string();

                // Highlight the match
                if snippet_start > 0 {
                    snippet = format!("...{}", snippet);
                }
                if snippet_end < note.content.len() {
                    snippet = format!("{}...", snippet);
                }

                results.push(SearchResult {
                    note: note.clone(),
                    snippet,
                    position: real_pos,
                });

                // Continue searching after this match
                content_pos = real_pos + query.len();

                // Limit to 3 results per note to prevent excessive matches
                if results.len() % 3 == 0 && results.len() > 0 {
                    break;
                }
            }
        }

        self.results = results;
        self.selected_index = if self.results.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    /// Display the search UI
    pub fn ui(&mut self, ui: &mut Ui) -> Option<(String, usize)> {
        let mut clicked_result = None;

        ui.horizontal(|ui| {
            ui.label("Search:");
            if ui.text_edit_singleline(&mut self.query).changed() {
                // Search will be performed externally to avoid borrow issues
            }

            if ui.button("🔍").clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                // Search button clicked or Enter pressed - will be handled externally
            }

            if ui.button("✕").clicked() {
                self.query.clear();
                self.results.clear();
                self.selected_index = None;
            }
        });

        // Show search results
        if !self.results.is_empty() {
            ui.separator();
            ui.label(format!("Found {} results", self.results.len()));

            egui::ScrollArea::vertical().show(ui, |ui| {
                for (index, result) in self.results.iter().enumerate() {
                    ui.group(|ui| {
                        let is_selected = self.selected_index == Some(index);
                        let title_text = if is_selected {
                            RichText::new(&result.note.title())
                                .strong()
                                .color(ui.visuals().selection.stroke.color)
                        } else {
                            RichText::new(&result.note.title()).strong()
                        };

                        if ui.link(title_text).clicked() {
                            self.selected_index = Some(index);
                            clicked_result = Some((result.note.id.clone(), result.position));
                        }

                        ui.label(&result.snippet);
                    });
                }
            });
        } else if !self.query.is_empty() {
            ui.separator();
            ui.label("No results found");
        }

        clicked_result
    }
}
