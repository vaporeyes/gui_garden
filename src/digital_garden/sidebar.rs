use super::note::Note;
use super::note_directory::{Folder, NoteDirectory};
use egui::{RichText, Ui};
use std::sync::Arc;

/// User actions surfaced by one frame of sidebar UI. The caller handles
/// these after the borrow on `NoteDirectory` ends.
#[derive(Default)]
pub struct SidebarAction {
    pub clicked_note: Option<String>,
    /// The user asked to clear the active tag filter.
    pub clear_tag: bool,
    /// The user clicked a tag in the tag cloud — set it as the active
    /// filter.
    pub selected_tag: Option<String>,
}

/// State for the sidebar
pub struct Sidebar {
    /// Currently selected note ID
    pub selected_note: Option<String>,

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

    /// Show the sidebar.
    ///
    /// `active_tag`: if `Some`, show only notes carrying that tag (overrides
    /// folder view and text search) plus a clear-filter banner. Setting it
    /// to `None` on the caller's side via the returned action clears it.
    ///
    /// `recent_ids`: most-recently-visited note ids (excluding current), from
    /// `History::recent`. Rendered as a "Recent" section above the folder tree.
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        note_directory: &NoteDirectory,
        active_tag: Option<&str>,
        recent_ids: &[String],
    ) -> SidebarAction {
        let mut action = SidebarAction::default();
        let accent = ui.visuals().selection.stroke.color;

        // Active tag filter banner (takes precedence over text search).
        if let Some(tag) = active_tag {
            ui.add_space(6.0);
            egui::Frame::NONE
                .fill(accent.linear_multiply(0.12))
                .inner_margin(egui::Margin::same(6))
                .corner_radius(egui::CornerRadius::same(4))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("filtering by #{}", tag))
                                .small()
                                .color(accent),
                        );
                        if ui.small_button("clear").clicked() {
                            action.clear_tag = true;
                        }
                    });
                });
            ui.add_space(6.0);
            section_label(ui, &format!("tagged #{}", tag), accent);
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for note in note_directory.notes_with_tag(tag) {
                        if note.is_draft() {
                            continue;
                        }
                        let is_selected = self.selected_note.as_deref() == Some(&note.id);
                        let text = if is_selected {
                            RichText::new(note.title()).strong().color(accent)
                        } else {
                            RichText::new(note.title())
                        };
                        if ui.link(text).clicked() {
                            self.selected_note = Some(note.id.clone());
                            action.clicked_note = Some(note.id.clone());
                        }
                    }
                });
            return action;
        }

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

        // Tag cloud — collapsible so it doesn't steal too much vertical
        // space from the folder tree on small windows. Chip size scales
        // with tag frequency.
        let tag_freqs = note_directory.tag_frequencies();
        if !tag_freqs.is_empty() {
            egui::CollapsingHeader::new(
                RichText::new("TAGS").small().strong().color(accent),
            )
            .id_salt("sidebar_tag_cloud")
            .default_open(false)
            .show(ui, |ui| {
                let mut tags: Vec<(&String, &u32)> = tag_freqs.iter().collect();
                tags.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
                let max_count = tags.first().map(|(_, c)| **c).unwrap_or(1) as f32;

                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                    for (tag, count) in tags {
                        let weight = (*count as f32 / max_count).max(0.2);
                        // Font size scales with frequency (12 → 16 px).
                        let size = 12.0 + 4.0 * weight;
                        let chip = RichText::new(format!("#{}", tag))
                            .size(size)
                            .color(accent)
                            .background_color(accent.linear_multiply(0.12));
                        let resp = ui
                            .add(egui::Label::new(chip).sense(egui::Sense::click()))
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text(format!("{} ({} notes)", tag, count));
                        if resp.clicked() {
                            action.selected_tag = Some(tag.clone());
                        }
                    }
                });
            });
        }

        ui.add_space(6.0);
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
                            action.clicked_note = Some(note.id.clone());
                        }
                    }
                });
        } else {
            // Recent notes — only shown when the search box is empty
            // (search takes precedence as the active filter).
            if !recent_ids.is_empty() {
                section_label(ui, "recent", accent);
                for id in recent_ids {
                    if let Some(note) = note_directory.get_note(id) {
                        let is_selected = self.selected_note.as_deref() == Some(id);
                        let text = if is_selected {
                            RichText::new(note.title()).strong().color(accent)
                        } else {
                            RichText::new(note.title())
                        };
                        if ui
                            .add(egui::Label::new(text).sense(egui::Sense::click()))
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            action.clicked_note = Some(id.clone());
                        }
                    }
                }
                ui.add_space(8.0);
            }

            section_label(ui, "notes", accent);
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if let Some(click) =
                        self.show_folder(ui, &note_directory.folder_structure, note_directory, 0)
                    {
                        action.clicked_note = Some(click);
                    }
                });
        }

        action
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
