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

    /// Update the app UI
    pub fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Detect mobile viewport
        let screen_size = ctx.input(|i| i.screen_rect().size());
        self.is_mobile = screen_size.x < 768.0;

        // Apply theme
        self.theme_manager.apply_theme(ctx);

        // Top panel with navigation and search
        TopBottomPanel::top("top_panel").show(ctx, |ui| {
            self.top_panel_ui(ui);
        });

        // Sidebar (hidden on mobile if toggled off)
        if !self.is_mobile || self.show_sidebar {
            SidePanel::left("sidebar")
                .resizable(true)
                .default_width(250.0)
                .width_range(200.0..=400.0)
                .show(ctx, |ui| {
                    if let Some(clicked_note) =
                        self.sidebar.ui(ui, self.note_directory.as_ref().unwrap())
                    {
                        self.load_note(&clicked_note);
                    }
                });
        }

        // Main content area
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(note) = self.current_note.clone() {
                self.render_note(ui, &note);
            } else {
                ui.centered_and_justified(|ui| {
                    ui.heading("No note selected");
                });
            }
        });

        // Graph modal
        if self.graph_open {
            let mut graph_open = self.graph_open;
            let clicked_node = Window::new("Graph View")
                .open(&mut graph_open)
                .resizable(true)
                .default_width(600.0)
                .default_height(400.0)
                .show(ctx, |ui| {
                    if let Some(directory) = &self.note_directory {
                        self.graph_view.ui(ui, directory)
                    } else {
                        None
                    }
                })
                .map(|inner| inner.inner)
                .flatten();
                
            self.graph_open = graph_open;
            if let Some(node_id) = clicked_node {
                if let Some(node_id) = node_id {
                    self.load_note(&node_id);
                }
            }
        }

        // Search modal
        if self.search_open {
            let mut search_open = self.search_open;
            Window::new("Search")
                .resizable(true)
                .default_width(500.0)
                .default_height(400.0)
                .show(ctx, |ui| {
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        search_open = false;
                    }

                    if self.search.ui(ui).is_some() && self.note_directory.is_some() {
                        self.search.search(self.note_directory.as_ref().unwrap());
                    }

                    if let Some((note_id, _position)) = self.search.ui(ui) {
                        self.load_note(&note_id);
                        search_open = false;
                    }
                });
            self.search_open = search_open;
        }

        // Settings modal
        if self.settings_open {
            Window::new("Settings")
                .open(&mut self.settings_open)
                .resizable(true)
                .default_width(400.0)
                .default_height(300.0)
                .show(ctx, |ui| {
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
                                    self.theme_manager.apply_theme(ctx);
                                }
                            }
                        });
    
                    if ui.button("Toggle Dark/Light Mode").clicked() {
                        self.theme_manager.toggle_dark_mode(ctx);
                    }
                });
        }
    }

    /// Render the top panel UI
    fn top_panel_ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            // Mobile sidebar toggle
            if self.is_mobile {
                if ui.button(if self.show_sidebar { "≡" } else { "≡" }).clicked() {
                    self.show_sidebar = !self.show_sidebar;
                }
            }

            // Back/forward navigation
            if ui.button("←").clicked() {
                self.navigate_back();
            }

            if ui.button("→").clicked() {
                self.navigate_forward();
            }

            // Current note title
            if let Some(note) = &self.current_note {
                ui.heading(note.title());
            } else {
                ui.heading("Digital Garden");
            }

            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                // Search button
                if ui.button("🔍").clicked() {
                    self.search_open = true;
                }

                // Graph button
                if ui.button("📊").clicked() {
                    self.graph_open = true;
                }

                // Settings button
                if ui.button("⚙").clicked() {
                    self.settings_open = true;
                }
            });
        });
    }

    /// Render a note
    fn render_note(&mut self, ui: &mut Ui, note: &Arc<Note>) {
        let directory = match &self.note_directory {
            Some(dir) => dir,
            None => return,
        };

        // Note metadata
        ui.horizontal(|ui| {
            if let Some(created) = &note.frontmatter.created {
                ui.label(format!("Created: {}", created.format("%Y-%m-%d")));
            }

            if let Some(updated) = &note.frontmatter.updated {
                ui.label(format!("Updated: {}", updated.format("%Y-%m-%d")));
            }

            if !note.frontmatter.tags.is_empty() {
                ui.label("Tags:");
                for tag in &note.frontmatter.tags {
                    if ui.link(format!("#{}", tag)).clicked() {
                        // TODO: Filter by tag
                    }
                }
            }
        });

        ui.separator();

        // Just keep track of the link and load it afterwards
        let parsed = markdown_parser::parse_markdown(&note.content);
        let mut link_selected: Option<String> = None;
        let mut on_link_click = |target_id: &str| {
            link_selected = Some(target_id.to_string());
        };

        ScrollArea::vertical().show(ui, |ui| {
            markdown_parser::render_markdown(ui, &parsed, directory, &note.id, &mut on_link_click);
            markdown_parser::process_internal_links(ui, note, directory, &mut on_link_click);
        });

        if let Some(target_id) = link_selected {
            self.load_note(&target_id);
        }
    }
}
