mod graph_view;
mod history;
mod markdown_parser;
mod note;
mod note_directory;
mod search;
mod sidebar;
mod theme;
mod watcher;

use history::History;
use watcher::DirectoryWatcher;

pub use graph_view::*;
pub use note::*;
pub use note_directory::*;
pub use search::*;
pub use sidebar::*;
pub use theme::*;

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

    /// Navigation history (browser-style back/forward with dedupe + cap).
    #[serde(skip)]
    history: History,

    /// Keyboard-shortcut help popup is open.
    #[serde(skip)]
    shortcuts_open: bool,

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

    /// Currently active tag filter (None = show everything). Clicking a
    /// tag chip in the note header or the tags pane sets this; clicking
    /// "clear" in the tag pane drops it.
    #[serde(skip)]
    active_tag: Option<String>,

    /// Per-note scroll offsets (y). Stashed whenever a note is rendered
    /// and restored on revisit — back/forward now return you to where you
    /// were reading, like a browser.
    #[serde(skip)]
    note_scroll_offsets: std::collections::HashMap<String, f32>,

    /// Optional filesystem watcher for the active notes directory. When the
    /// user edits a note in another editor (vim, Obsidian, VSCode), we pick
    /// up the change and rebuild `note_directory` in place.
    #[serde(skip)]
    watcher: Option<DirectoryWatcher>,

    /// Debounce window (ms) for the watcher. Persisted so the user's
    /// preference survives restarts.
    hot_reload_debounce_ms: u64,
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
            history: History::new(),
            shortcuts_open: false,
            search_open: false,
            graph_open: false,
            settings_open: false,
            show_sidebar: true,
            is_mobile: false,
            active_tag: None,
            note_scroll_offsets: std::collections::HashMap::new(),
            watcher: None,
            hot_reload_debounce_ms: watcher::DEFAULT_DEBOUNCE_MS,
        }
    }
}

impl DigitalGarden {
    /// Apply the current theme to the given context. Exposed so the outer app
    /// can tint its own chrome with the same palette at startup, before the
    /// Digital Garden window has ever been opened.
    pub fn apply_theme(&self, ctx: &egui::Context) {
        self.theme_manager.apply_theme(ctx);
    }

    /// Drop the currently-loaded directory and every derived piece of state:
    /// note directory, watcher thread, current note, navigation history,
    /// tag filter, and graph nodes. Used by the outer app's "Change notes
    /// folder" button, which previously only cleared `note_directory` and
    /// left the filesystem watcher leaking.
    pub fn close_directory(&mut self) {
        self.notes_path = None;
        self.note_directory = None;
        self.watcher = None;
        self.current_note = None;
        self.history.clear();
        self.active_tag = None;
        self.graph_view.nodes.clear();
        self.sidebar.selected_note = None;
    }

    /// Set the notes directory
    pub fn set_notes_directory<P: AsRef<Path>>(&mut self, path: P) -> Result<(), String> {
        let path = path.as_ref().to_path_buf();

        match NoteDirectory::new(&path) {
            Ok(directory) => {
                self.notes_path = Some(path.clone());
                self.note_directory = Some(directory);
                self.graph_view
                    .build_graph(self.note_directory.as_ref().unwrap());

                // Fresh directory → fresh history. Previously, stale
                // `history_position` from a prior session could index out
                // of bounds on the new, shorter history.
                self.history.clear();
                self.watcher = DirectoryWatcher::new(&path, self.hot_reload_debounce_ms);

                // Load the index note if it exists, otherwise load the first note
                if self.load_note("index").is_none() {
                    if let Some(directory) = &self.note_directory {
                        if let Some(first_note) = directory.published_notes().first() {
                            let first_id = first_note.id.clone();
                            self.current_note = Some(first_note.clone());
                            self.history.reset(first_id);
                        }
                    }
                }

                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Re-read the notes directory from disk, preserving the currently
    /// viewed note (if it still exists). Called by the watcher when it
    /// detects external edits.
    fn reload_directory(&mut self) {
        let Some(path) = self.notes_path.clone() else {
            return;
        };
        let current_id = self.current_note.as_ref().map(|n| n.id.clone());
        match NoteDirectory::new(&path) {
            Ok(directory) => {
                self.note_directory = Some(directory);
                // Preserve graph positions across hot-reloads; only re-lay-out
                // when notes are added or removed, not on content edits.
                self.graph_view
                    .refresh_from_directory(self.note_directory.as_ref().unwrap());
                // Refresh `current_note` so the body re-renders with the
                // updated content. Fall back to the first note if the old
                // one was deleted.
                if let Some(id) = current_id {
                    if let Some(note) = self
                        .note_directory
                        .as_ref()
                        .and_then(|d| d.get_note(&id))
                    {
                        self.current_note = Some(note);
                    } else if let Some(first) = self
                        .note_directory
                        .as_ref()
                        .and_then(|d| d.published_notes().first().cloned())
                    {
                        self.current_note = Some(first);
                    } else {
                        self.current_note = None;
                    }
                }
            }
            Err(err) => {
                eprintln!("reload failed for {:?}: {}", path, err);
            }
        }
    }

    /// Load a note by id or wiki-link query. Delegates resolution to
    /// `NoteDirectory::resolve_link`, which handles exact id, case-insensitive
    /// id, slug, and title-match fallbacks (and filters out drafts).
    pub fn load_note(&mut self, note_id: &str) -> Option<Arc<Note>> {
        let directory = self.note_directory.as_ref()?;
        let note = directory.resolve_link(note_id)?;

        // History and sidebar are keyed by the *canonical* id, not the
        // raw query — so a wiki-link written as "Elegy Campaign Player"
        // still back-navigates correctly after resolution.
        let canonical_id = note.id.clone();
        self.current_note = Some(note.clone());
        self.history.push(canonical_id.clone());
        self.sidebar.selected_note = Some(canonical_id.clone());

        // Web deep-link: mirror the current note into the URL fragment so
        // reloading the page or sharing the URL lands on the same note.
        #[cfg(target_arch = "wasm32")]
        set_url_fragment(&canonical_id);

        Some(note)
    }

    /// Navigate back in history
    pub fn navigate_back(&mut self) -> Option<Arc<Note>> {
        let target = self.history.back()?.to_string();
        self.load_current_from_directory(&target)
    }

    /// Navigate forward in history
    pub fn navigate_forward(&mut self) -> Option<Arc<Note>> {
        let target = self.history.forward()?.to_string();
        self.load_current_from_directory(&target)
    }

    /// Internal helper for back/forward: swap in the note at `id` without
    /// touching history (since history already moved the cursor).
    fn load_current_from_directory(&mut self, id: &str) -> Option<Arc<Note>> {
        let note = self.note_directory.as_ref()?.get_note(id)?;
        self.current_note = Some(note.clone());
        self.sidebar.selected_note = Some(id.to_string());
        Some(note)
    }

    /// Render the digital garden inside the given `Ui`. Panels use `show_inside`
    /// so everything stays scoped to the containing window instead of leaking
    /// out to the root context (which used to cause duplicate-ID warnings and
    /// visual overlap with the outer app chrome).
    pub fn ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();

        // Hot-reload poll. If the watcher has seen a debounced batch of
        // .md changes on disk, re-scan the directory in place so external
        // edits (vim / Obsidian / VS Code) show up without relaunching.
        let should_reload = self
            .watcher
            .as_mut()
            .map(|w| w.consume_if_ready())
            .unwrap_or(false);
        if should_reload {
            self.reload_directory();
            ctx.request_repaint();
        }

        // Keyboard shortcuts — only when no TextEdit is focused so typing
        // in the sidebar filter or search box doesn't trigger navigation.
        let anything_focused = ctx.memory(|m| m.focused().is_some());
        if !anything_focused {
            ctx.input(|i| {
                if i.key_pressed(egui::Key::Slash) && !i.modifiers.any() {
                    self.search_open = true;
                }
                if i.key_pressed(egui::Key::Questionmark) {
                    self.shortcuts_open = true;
                }
                if i.modifiers.command && i.key_pressed(egui::Key::OpenBracket) {
                    if let Some(id) = self.history.back().map(|s| s.to_string()) {
                        // `back()` already moved the cursor; refresh the current note.
                        let _ = self.load_current_from_directory(&id);
                    }
                }
                if i.modifiers.command && i.key_pressed(egui::Key::CloseBracket) {
                    if let Some(id) = self.history.forward().map(|s| s.to_string()) {
                        let _ = self.load_current_from_directory(&id);
                    }
                }
            });
        }

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

        // Sidebar (hidden on narrow viewports if toggled off). We return
        // actions out of the closure and handle them *after* the directory
        // borrow ends, so `load_note` can take `&mut self` cleanly.
        let sidebar_action: sidebar::SidebarAction = if !self.is_mobile || self.show_sidebar {
            SidePanel::left("dg_sidebar")
                .resizable(true)
                .default_width(240.0)
                .width_range(200.0..=380.0)
                .show_inside(ui, |ui| match self.note_directory.as_ref() {
                    Some(directory) => {
                        let recent = self.history.recent(5);
                        self.sidebar.ui(
                            ui,
                            directory,
                            self.active_tag.as_deref(),
                            &recent,
                        )
                    }
                    None => sidebar::SidebarAction::default(),
                })
                .inner
        } else {
            sidebar::SidebarAction::default()
        };
        if sidebar_action.clear_tag {
            self.active_tag = None;
        }
        if let Some(tag) = sidebar_action.selected_tag {
            self.active_tag = Some(tag);
        }
        if let Some(clicked_note) = sidebar_action.clicked_note {
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

                    ui.add_space(12.0);
                    ui.heading("Hot reload");
                    ui.label(
                        egui::RichText::new(
                            "Debounce window before reacting to external file changes.",
                        )
                        .small()
                        .weak(),
                    );
                    let mut debounce = self.hot_reload_debounce_ms;
                    if ui
                        .add(
                            egui::Slider::new(&mut debounce, 50..=2000)
                                .suffix(" ms")
                                .text("debounce"),
                        )
                        .changed()
                    {
                        self.hot_reload_debounce_ms = debounce;
                        if let Some(w) = self.watcher.as_mut() {
                            w.set_debounce_ms(debounce);
                        }
                    }
                });
            self.settings_open = settings_open;
        }

        // Shortcuts reference — small, read-only popup. Listed here so the
        // user isn't left guessing that `/` opens search or Cmd+[/] navigates.
        if self.shortcuts_open {
            let accent = self.theme_manager.accent();
            let mut shortcuts_open = self.shortcuts_open;
            Window::new("Keyboard Shortcuts")
                .open(&mut shortcuts_open)
                .resizable(false)
                .default_width(320.0)
                .show(&ctx, |ui| {
                    let shortcuts: &[(&str, &str)] = &[
                        ("/", "Open search"),
                        ("Esc", "Close search / dismiss modal"),
                        ("Cmd+[", "Navigate back"),
                        ("Cmd+]", "Navigate forward"),
                        ("↑ / ↓", "Move between search results"),
                        ("Enter", "Open selected search result"),
                    ];
                    egui::Grid::new("dg_shortcuts_grid")
                        .num_columns(2)
                        .spacing([18.0, 6.0])
                        .show(ui, |ui| {
                            for (key, desc) in shortcuts {
                                ui.label(
                                    egui::RichText::new(*key)
                                        .monospace()
                                        .strong()
                                        .color(accent),
                                );
                                ui.label(*desc);
                                ui.end_row();
                            }
                        });
                });
            self.shortcuts_open = shortcuts_open;
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

            let back_enabled = self.history.can_go_back();
            let forward_enabled = self.history.can_go_forward();

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
                if ui.button("?").on_hover_text("Keyboard shortcuts").clicked() {
                    self.shortcuts_open = true;
                }
                if ui.button("📊").on_hover_text("Graph view").clicked() {
                    self.graph_open = true;
                }
                if ui.button("🔍").on_hover_text("Search").clicked() {
                    self.search_open = true;
                }
                // Export — only meaningful when a note is showing.
                if let Some(note) = self.current_note.clone() {
                    if ui
                        .button("📋")
                        .on_hover_text("Copy current note as HTML")
                        .clicked()
                    {
                        let html = markdown_parser::markdown_to_html(&note.content);
                        ui.ctx().copy_text(html);
                    }
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
        let mut tag_selected: Option<String> = None;

        // Restore the scroll position we saved the last time this note was
        // rendered. For a fresh note, the map lookup misses and the area
        // opens at the top as usual.
        let saved_offset = self.note_scroll_offsets.get(&note.id).copied().unwrap_or(0.0);
        let scroll_output = ScrollArea::vertical()
            .auto_shrink([false, false])
            .vertical_scroll_offset(saved_offset)
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

                        // Description (Astro's `description` frontmatter field)
                        if let Some(desc) = &note.frontmatter.description {
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(desc)
                                    .size(16.0)
                                    .italics()
                                    .color(muted),
                            );
                        }

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

                        // Tag chips — clickable, set the active tag filter.
                        if !note.frontmatter.tags.is_empty() {
                            ui.add_space(10.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                for tag in &note.frontmatter.tags {
                                    let chip = egui::RichText::new(format!("#{}", tag))
                                        .small()
                                        .color(accent)
                                        .background_color(chip_bg);
                                    let resp = ui
                                        .add(egui::Label::new(chip).sense(egui::Sense::click()))
                                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                                        .on_hover_text(format!(
                                            "Show all notes tagged #{}",
                                            tag
                                        ));
                                    if resp.clicked() {
                                        tag_selected = Some(tag.clone());
                                    }
                                }
                            });
                        }

                        ui.add_space(20.0);
                        ui.separator();
                        ui.add_space(16.0);

                        // Body
                        let mut on_link_click = |target_id: &str| {
                            link_selected = Some(target_id.to_string());
                        };
                        markdown_parser::render(
                            ui,
                            &note.content,
                            &directory,
                            &mut on_link_click,
                        );
                        markdown_parser::render_embeds(
                            ui,
                            note,
                            &directory,
                            &mut on_link_click,
                        );

                        // Backlinks footer
                        let backlinks = directory.backlinks(&note.id);
                        if !backlinks.is_empty() {
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
                            for source_id in backlinks {
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

        // Remember where the user had scrolled in this note, so navigating
        // away and back restores their position instead of snapping to top.
        let scroll_y = scroll_output.state.offset.y;
        self.note_scroll_offsets
            .insert(note.id.clone(), scroll_y);

        // Reading-progress indicator: a slim accent bar pinned to the very
        // top of the article pane, width proportional to scroll progress.
        // Driven by the ScrollArea's reported content + viewport heights.
        let content_h = scroll_output.content_size.y;
        let viewport = scroll_output.inner_rect;
        let viewport_h = viewport.height();
        let max_scroll = (content_h - viewport_h).max(1.0);
        let progress = (scroll_y / max_scroll).clamp(0.0, 1.0);
        if content_h > viewport_h {
            let bar_rect = egui::Rect::from_min_size(
                viewport.left_top(),
                egui::vec2(viewport.width() * progress, 3.0),
            );
            ui.painter().rect_filled(bar_rect, 0.0, accent);
        }

        if let Some(target_id) = link_selected {
            self.load_note(&target_id);
        }
        if let Some(tag) = tag_selected {
            self.active_tag = Some(tag);
        }
    }

    /// Read any incoming `#note-id` URL fragment on wasm and try to load
    /// that note. Called from `TemplateApp::new` after the directory is
    /// ready. No-op on native.
    pub fn apply_url_fragment_if_any(&mut self) {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(id) = read_url_fragment() {
                let trimmed = id.trim_start_matches('#');
                if !trimmed.is_empty() {
                    let _ = self.load_note(trimmed);
                }
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn read_url_fragment() -> Option<String> {
    let window = web_sys::window()?;
    let hash = window.location().hash().ok()?;
    if hash.is_empty() {
        None
    } else {
        Some(
            percent_decode(hash.trim_start_matches('#'))
                .unwrap_or_else(|| hash.trim_start_matches('#').to_string()),
        )
    }
}

#[cfg(target_arch = "wasm32")]
fn set_url_fragment(id: &str) {
    if let Some(window) = web_sys::window() {
        // `replaceState` avoids creating a new browser-history entry per
        // note navigation — our in-app history already handles back/forward.
        let new_url = format!("#{}", id);
        if let Ok(history) = window.history() {
            let _ = history.replace_state_with_url(
                &wasm_bindgen::JsValue::NULL,
                "",
                Some(&new_url),
            );
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn percent_decode(s: &str) -> Option<String> {
    // Light-weight percent-decoding — avoids pulling in the `percent-encoding`
    // crate for a single use-site. Accepts plain ASCII pairs, leaves invalid
    // sequences as-is.
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                out.push(byte as char);
                i += 3;
                continue;
            }
        }
        out.push(b as char);
        i += 1;
    }
    Some(out)
}
