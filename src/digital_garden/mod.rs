mod frontmatter;
mod graph_view;
mod markdown_parser;
mod note;
mod note_directory;
mod search;
mod sidebar;
mod theme;

pub use frontmatter::*;
pub use graph_view::*;
pub use markdown_parser::*;
pub use note::*;
pub use note_directory::*;
pub use search::*;
pub use sidebar::*;
pub use theme::*;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::{Layout, ScrollArea, SidePanel, TopBottomPanel, Ui, Window};
use serde::{Deserialize, Serialize};

/// Main digital garden app
#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct DigitalGarden {
    /// Path to the notes directory
    #[serde(skip)]
    notes_path: Option<PathBuf>,

    /// Note directory
    #[serde(skip)]
    pub note_directory: Option<NoteDirectory>,

    /// Currently displayed note
    #[serde(skip)]
    current_note: Option<Arc<Note>>,

    /// Sidebar state
    #[serde(skip)]
    sidebar: Sidebar,

    /// Graph view
    #[serde(skip)]
    graph_view: GraphView,

    /// Theme manager
    #[serde(skip)]
    theme_manager: ThemeManager,

    /// Search manager
    #[serde(skip)]
    search: Search,

    /// Navigation history
    #[serde(skip)]
    history: Vec<String>,

    /// Current position in history
    #[serde(skip)]
    history_position: usize,

    /// Search dialog is open
    #[serde(skip)]
    search_open: bool,

    /// Graph dialog is open
    #[serde(skip)]
    graph_open: bool,

    /// Settings dialog is open
    #[serde(skip)]
    settings_open: bool,

    /// Show sidebar (mobile toggle)
    #[serde(skip)]
    show_sidebar: bool,

    /// Mobile viewport
    #[serde(skip)]
    is_mobile: bool,
}

impl Default for DigitalGarden {
    fn default() -> Self {
        Self {
            notes_path: None,
            note_directory: None,
            current_note: None,
            sidebar: Sidebar::new(),
            graph_view: GraphView::new(),
            theme_manager: ThemeManager::new(),
            search: Search::new(),
            history: Vec::new(),
            history_position: 0,
            search_open: false,
            graph_open: false,
            settings_open: false,
            show_sidebar: true,
            is_mobile: false,
        }
    }
}

impl DigitalGarden {
    /// Create a new digital garden
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply the current theme to the given context. Exposed so the outer app
    /// can tint its own chrome with the same palette at startup, before the
    /// Digital Garden window has ever been opened.
    pub fn apply_theme(&self, ctx: &egui::Context) {
        self.theme_manager.apply_theme(ctx);
    }

    /// Set the notes directory
    pub fn set_notes_directory<P: AsRef<Path>>(&mut self, path: P) -> Result<(), String> {
        let path = path.as_ref().to_path_buf();

        match NoteDirectory::new(&path) {
            Ok(directory) => {
                self.notes_path = Some(path);
                self.note_directory = Some(directory);
                self.graph_view
                    .build_graph(self.note_directory.as_ref().unwrap());

                // Load the index note if it exists, otherwise load the first note
                if self.load_note("index").is_none() {
                    if let Some(directory) = &self.note_directory {
                        if let Some(first_note) = directory.published_notes().first() {
                            self.current_note = Some(first_note.clone());
                            self.history = vec![first_note.id.clone()];
                        }
                    }
                }

                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Load a note by ID
    pub fn load_note(&mut self, note_id: &str) -> Option<Arc<Note>> {
        if let Some(directory) = &self.note_directory {
            if let Some(note) = directory.get_note(note_id) {
                // Don't load draft notes
                if note.is_draft() {
                    return None;
                }

                self.current_note = Some(note.clone());

                // Update history
                if self.history.is_empty() || self.history[self.history_position] != note_id {
                    // Truncate history if we're not at the end
                    if self.history_position < self.history.len() - 1 {
                        self.history.truncate(self.history_position + 1);
                    }

                    self.history.push(note_id.to_string());
                    self.history_position = self.history.len() - 1;
                }

                // Update sidebar selection
                self.sidebar.selected_note = Some(note_id.to_string());

                return Some(note);
            }
        }

        None
    }

    /// Load a note by slug
    pub fn load_note_by_slug(&mut self, slug: &str) -> Option<Arc<Note>> {
        if let Some(directory) = &self.note_directory {
            if let Some(note) = directory.get_note_by_slug(slug) {
                // Don't load draft notes
                if note.is_draft() {
                    return None;
                }

                self.current_note = Some(note.clone());

                // Update history
                if self.history.is_empty() || self.history[self.history_position] != note.id {
                    // Truncate history if we're not at the end
                    if self.history_position < self.history.len() - 1 {
                        self.history.truncate(self.history_position + 1);
                    }

                    self.history.push(note.id.clone());
                    self.history_position = self.history.len() - 1;
                }

                // Update sidebar selection
                self.sidebar.selected_note = Some(note.id.clone());

                return Some(note);
            }
        }

        None
    }

    /// Navigate back in history
    pub fn navigate_back(&mut self) -> Option<Arc<Note>> {
        if self.history_position > 0 {
            self.history_position -= 1;
            let note_id = &self.history[self.history_position];

            if let Some(directory) = &self.note_directory {
                if let Some(note) = directory.get_note(note_id) {
                    self.current_note = Some(note.clone());
                    self.sidebar.selected_note = Some(note_id.clone());
                    return Some(note);
                }
            }
        }

        None
    }

    /// Navigate forward in history
    pub fn navigate_forward(&mut self) -> Option<Arc<Note>> {
        if self.history_position < self.history.len() - 1 {
            self.history_position += 1;
            let note_id = &self.history[self.history_position];

            if let Some(directory) = &self.note_directory {
                if let Some(note) = directory.get_note(note_id) {
                    self.current_note = Some(note.clone());
                    self.sidebar.selected_note = Some(note_id.clone());
                    return Some(note);
                }
            }
        }

        None
    }

    /// Render the digital garden inside the given `Ui`. Panels use `show_inside`
    /// so everything stays scoped to the containing window instead of leaking
    /// out to the root context (which used to cause duplicate-ID warnings and
    /// visual overlap with the outer app chrome).
    pub fn ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();

        // Viewport check against the *containing* ui, not the whole screen,
        // so collapsing behavior responds to the window's actual width.
        self.is_mobile = ui.available_width() < 720.0;

        // Top panel with navigation and search — namespaced ID to avoid
        // collision with the outer TemplateApp's "top_panel".
        TopBottomPanel::top("dg_top_panel")
            .show_separator_line(false)
            .show_inside(ui, |ui| {
                ui.add_space(4.0);
                self.top_panel_ui(ui);
                ui.add_space(4.0);
            });

        // Sidebar (hidden on narrow viewports if toggled off). We return the
        // clicked note id out of the closure and handle navigation *after* the
        // directory borrow ends, so `load_note` can take `&mut self` cleanly.
        let sidebar_click: Option<String> = if !self.is_mobile || self.show_sidebar {
            SidePanel::left("dg_sidebar")
                .resizable(true)
                .default_width(240.0)
                .width_range(200.0..=380.0)
                .show_inside(ui, |ui| match self.note_directory.as_ref() {
                    Some(directory) => self.sidebar.ui(ui, directory),
                    None => None,
                })
                .inner
        } else {
            None
        };
        if let Some(clicked_note) = sidebar_click {
            self.load_note(&clicked_note);
        }

        // Main content area
        egui::CentralPanel::default().show_inside(ui, |ui| {
            if let Some(note) = self.current_note.clone() {
                self.render_note(ui, &note);
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        egui::RichText::new("Select a note from the sidebar")
                            .weak()
                            .size(16.0),
                    );
                });
            }
        });

        // Modal windows still live at the context level — they're overlays
        // and meant to float above everything.
        if self.graph_open {
            let mut graph_open = self.graph_open;
            let clicked_node = Window::new("Graph View")
                .open(&mut graph_open)
                .resizable(true)
                .default_width(600.0)
                .default_height(400.0)
                .show(&ctx, |ui| {
                    if let Some(directory) = &self.note_directory {
                        self.graph_view.ui(ui, directory)
                    } else {
                        None
                    }
                })
                .and_then(|inner| inner.inner)
                .flatten();

            self.graph_open = graph_open;
            if let Some(node_id) = clicked_node {
                self.load_note(&node_id);
            }
        }

        // Search modal — single call to `search.ui`, with input handling and
        // result-click handling done from its single return value. The previous
        // double-call was the source of the "Second use of widget ID" warning.
        if self.search_open {
            let mut search_open = self.search_open;
            Window::new("Search")
                .resizable(true)
                .default_width(520.0)
                .default_height(440.0)
                .show(&ctx, |ui| {
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        search_open = false;
                    }

                    let clicked = self.search.ui(ui);
                    if let Some(directory) = self.note_directory.as_ref() {
                        // Re-run the search any time the query has changed;
                        // cheap given the in-memory corpus.
                        self.search.search(directory);
                    }
                    if let Some((note_id, _position)) = clicked {
                        self.load_note(&note_id);
                        search_open = false;
                    }
                });
            self.search_open = search_open;
        }

        // Settings modal
        if self.settings_open {
            let mut settings_open = self.settings_open;
            Window::new("Settings")
                .open(&mut settings_open)
                .resizable(true)
                .default_width(400.0)
                .default_height(300.0)
                .show(&ctx, |ui| {
                    ui.heading("Theme");

                    egui::ComboBox::from_label("Theme")
                        .selected_text(&self.theme_manager.current_theme.name)
                        .show_ui(ui, |ui| {
                            let themes = self.theme_manager.available_themes.clone();
                            for theme_name in &themes {
                                if ui
                                    .selectable_label(
                                        self.theme_manager.current_theme.name == *theme_name,
                                        theme_name,
                                    )
                                    .clicked()
                                {
                                    self.theme_manager.set_theme(theme_name);
                                    self.theme_manager.apply_theme(&ctx);
                                }
                            }
                        });

                    if ui.button("Toggle Dark/Light Mode").clicked() {
                        self.theme_manager.toggle_dark_mode(&ctx);
                    }
                });
            self.settings_open = settings_open;
        }
    }

    /// Render the top panel UI: nav controls on the left, action icons flush
    /// right. The current-note title lives in the article body, not up here,
    /// so the top bar stays compact and consistent across notes.
    fn top_panel_ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            // Mobile sidebar toggle
            if self.is_mobile && ui.button("≡").on_hover_text("Toggle sidebar").clicked() {
                self.show_sidebar = !self.show_sidebar;
            }

            let back_enabled = self.history_position > 0;
            let forward_enabled =
                !self.history.is_empty() && self.history_position < self.history.len() - 1;

            if ui
                .add_enabled(back_enabled, egui::Button::new("←"))
                .on_hover_text("Back")
                .clicked()
            {
                self.navigate_back();
            }
            if ui
                .add_enabled(forward_enabled, egui::Button::new("→"))
                .on_hover_text("Forward")
                .clicked()
            {
                self.navigate_forward();
            }

            ui.separator();
            ui.label(
                egui::RichText::new("digital garden")
                    .small()
                    .weak()
                    .italics(),
            );

            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("⚙").on_hover_text("Settings").clicked() {
                    self.settings_open = true;
                }
                if ui.button("📊").on_hover_text("Graph view").clicked() {
                    self.graph_open = true;
                }
                if ui.button("🔍").on_hover_text("Search").clicked() {
                    self.search_open = true;
                }
            });
        });
    }

    /// Render a note in article-style: accent-colored title, muted meta row,
    /// pill-shaped tag chips, and a reading-width body column.
    fn render_note(&mut self, ui: &mut Ui, note: &Arc<Note>) {
        let directory = match &self.note_directory {
            Some(dir) => dir.clone(),
            None => return,
        };
        let accent = self.theme_manager.accent();
        let muted = self.theme_manager.muted_text();
        let chip_bg = self.theme_manager.accent().linear_multiply(0.18);

        let mut link_selected: Option<String> = None;

        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.set_max_width(760.0);
                    ui.with_layout(Layout::top_down(egui::Align::LEFT), |ui| {
                        ui.add_space(24.0);

                        // Title
                        ui.label(
                            egui::RichText::new(note.title())
                                .size(30.0)
                                .strong()
                                .color(accent),
                        );
                        ui.add_space(4.0);

                        // Meta row: publication + updated timestamps
                        let mut meta_parts: Vec<String> = Vec::new();
                        if let Some(created) = &note.frontmatter.created {
                            meta_parts.push(format!("Published {}", created.format("%B %-d, %Y")));
                        }
                        if let Some(updated) = &note.frontmatter.updated {
                            meta_parts.push(format!("Updated {}", updated.format("%B %-d, %Y")));
                        }
                        if !meta_parts.is_empty() {
                            ui.label(
                                egui::RichText::new(meta_parts.join("  ·  "))
                                    .small()
                                    .color(muted),
                            );
                        }

                        // Tag chips
                        if !note.frontmatter.tags.is_empty() {
                            ui.add_space(10.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                for tag in &note.frontmatter.tags {
                                    let chip = egui::RichText::new(format!("#{}", tag))
                                        .small()
                                        .color(accent)
                                        .background_color(chip_bg);
                                    let resp = ui.add(
                                        egui::Label::new(chip).sense(egui::Sense::click()),
                                    );
                                    if resp.clicked() {
                                        // TODO: filter by tag — wired up here so the UI
                                        // affordance is honest about being interactive.
                                    }
                                }
                            });
                        }

                        ui.add_space(20.0);
                        ui.separator();
                        ui.add_space(16.0);

                        // Body
                        let parsed = markdown_parser::parse_markdown(&note.content);
                        let mut on_link_click = |target_id: &str| {
                            link_selected = Some(target_id.to_string());
                        };
                        markdown_parser::render_markdown(
                            ui,
                            &parsed,
                            &directory,
                            &note.id,
                            &mut on_link_click,
                        );
                        markdown_parser::process_internal_links(
                            ui,
                            note,
                            &directory,
                            &mut on_link_click,
                        );

                        // Backlinks footer
                        if !note.backlinks.is_empty() {
                            ui.add_space(28.0);
                            ui.separator();
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new("LINKED FROM")
                                    .small()
                                    .strong()
                                    .color(accent),
                            );
                            ui.add_space(6.0);
                            for source_id in &note.backlinks {
                                if let Some(source) = directory.get_note(source_id) {
                                    if ui.link(source.title()).clicked() {
                                        link_selected = Some(source.id.clone());
                                    }
                                }
                            }
                        }

                        ui.add_space(48.0);
                    });
                });
            });

        if let Some(target_id) = link_selected {
            self.load_note(&target_id);
        }
    }
}
