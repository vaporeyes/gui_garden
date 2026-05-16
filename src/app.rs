use crate::apps::clock_button;
use crate::apps::easy_mark;
use crate::command_palette::{
    CommandPalette, PaletteItem, PaletteResult, SystemAction, WindowKind, TOGGLE_SHORTCUT,
    WINDOW_SHORTCUTS,
};
use crate::digital_garden::DigitalGarden;
use eframe::egui;
use egui::Ui;
use std::path::PathBuf;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct TemplateApp {
    about_is_open: bool,
    calc_is_open: bool,
    events_is_open: bool,
    resume_is_open: bool,
    digital_garden_is_open: bool,
    projects_is_open: bool,
    canvas_is_open: bool,
    workouts_is_open: bool,
    binary_clock_is_open: bool,
    collections_is_open: bool,
    bezier_is_open: bool,
    palette_studio_is_open: bool,
    raycaster_is_open: bool,
    timestamp_converter_is_open: bool,
    wad_viewer_is_open: bool,
    #[serde(skip)]
    calculator: crate::apps::Calculator,
    #[serde(skip)]
    fractal_clock: crate::apps::FractalClock,
    #[serde(skip)]
    binary_clock: crate::apps::BinaryClock,
    #[serde(skip)]
    about_me: crate::about::AboutMe,
    #[serde(skip)]
    projects: crate::apps::Projects,
    #[serde(skip)]
    canvas_view: crate::apps::CanvasView,
    #[serde(skip)]
    workouts: crate::apps::Workouts,
    #[serde(skip)]
    collections: crate::apps::Collections,
    #[serde(skip)]
    bezier: crate::apps::BezierPlayground,
    #[serde(skip)]
    palette_studio: crate::apps::PaletteStudio,
    #[serde(skip)]
    raycaster: crate::apps::Raycaster,
    #[serde(skip)]
    timestamp_converter: crate::apps::TimestampConverter,
    #[serde(skip)]
    wad_viewer: crate::apps::WadViewer,
    /// Persisted path to the most-recently-used Doom WAD file.
    wad_path: String,
    /// DigitalGarden's runtime state is all `#[serde(skip)]` internally; the
    /// only field that persists is user preferences like the hot-reload
    /// debounce window, which survive restarts.
    digital_garden: DigitalGarden,
    /// Persisted path to the markdown notes directory. `~` is expanded at load time.
    notes_directory_path: String,
    /// Persisted path to `workouts.json`, re-loaded automatically on startup.
    workouts_path: String,
    /// Persisted path to the most-recently-used JSON Canvas file.
    canvas_path: String,
    /// In-memory error surfaced when the persisted notes directory fails
    /// to load at startup or via the Digital Garden's Load button. Not
    /// persisted — once the user fixes the path, they shouldn't see the
    /// stale message next launch.
    #[serde(skip)]
    notes_directory_error: Option<String>,
    output_event_history: std::collections::VecDeque<egui::output::OutputEvent>,
    /// In-flight confirmation request, if any. egui 0.30+ gives us proper
    /// `Modal`s (input-blocking, escape-to-dismiss); we use them for
    /// destructive ops that drop unrecoverable state.
    #[serde(skip)]
    pending_confirm: Option<PendingConfirm>,
    /// Cmd-K command palette. Self-contained widget; we just build its
    /// items list each frame and dispatch the action it returns.
    #[serde(skip)]
    command_palette: CommandPalette,
    /// Persisted active color-scheme index. The runtime live state is
    /// the `palette::ACTIVE_SCHEME` atomic; this field is the on-disk
    /// shadow that re-seeds the atomic on launch.
    palette_scheme_idx: u8,
}

/// Destructive actions that require a Modal confirmation before running.
#[derive(Clone, Copy)]
enum PendingConfirm {
    ChangeNotesFolder,
    ResetEguiMemory,
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            about_is_open: true,
            calc_is_open: false,
            events_is_open: false,
            resume_is_open: false,
            digital_garden_is_open: false,
            projects_is_open: false,
            canvas_is_open: false,
            workouts_is_open: false,
            binary_clock_is_open: false,
            collections_is_open: false,
            bezier_is_open: false,
            palette_studio_is_open: false,
            raycaster_is_open: false,
            timestamp_converter_is_open: false,
            wad_viewer_is_open: false,
            calculator: Default::default(),
            fractal_clock: Default::default(),
            binary_clock: Default::default(),
            about_me: Default::default(),
            projects: Default::default(),
            canvas_view: Default::default(),
            workouts: Default::default(),
            collections: Default::default(),
            bezier: Default::default(),
            palette_studio: Default::default(),
            raycaster: Default::default(),
            timestamp_converter: Default::default(),
            wad_viewer: Default::default(),
            wad_path: String::new(),
            digital_garden: DigitalGarden::default(),
            notes_directory_path: String::new(),
            workouts_path: String::new(),
            canvas_path: String::new(),
            notes_directory_error: None,
            output_event_history: Default::default(),
            pending_confirm: None,
            command_palette: CommandPalette::default(),
            palette_scheme_idx: 0,
        }
    }
}

impl TemplateApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customized the look at feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Custom font stack: Atkinson Hyperlegible for UI sans, Lora for
        // markdown body/heading via a custom `FontFamily::Name("Serif")`,
        // IBM Plex Mono for code blocks, the binary clock, and inline
        // code spans. All three are SIL OFL licensed.
        install_fonts(&cc.egui_ctx);

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        let mut app: Self = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        };

        // Restore the persisted color scheme into the runtime atomic so
        // `palette::accent_now()` returns the user's last choice on
        // first frame (otherwise the default `TimeOfDay` would briefly
        // flash before any UI sets it).
        crate::palette::set_active_scheme(crate::palette::Scheme::from_index(
            app.palette_scheme_idx,
        ));

        // Apply the digital-garden theme to the whole app up-front, so the
        // outer menu bar + sidebar + window chrome pick up the amber palette
        // even before the Digital Garden window is opened.
        app.digital_garden.apply_theme(&cc.egui_ctx);

        // Image loaders: enables `file://`, `http(s)://`, `data:` URIs in
        // `egui::Image::new(...)`. Used by the markdown renderer for
        // inline `![alt](src)` tags.
        egui_extras::install_image_loaders(&cc.egui_ctx);

        // Auto-load the persisted notes directory so the digital garden is ready
        // on launch without the user re-entering the path every session.
        let trimmed = app.notes_directory_path.trim();
        if !trimmed.is_empty() {
            let path = expand_path(trimmed);
            if let Err(err) = app.digital_garden.set_notes_directory(&path) {
                app.notes_directory_error =
                    Some(format!("Couldn't load {}: {}", path.display(), err));
            }
        }

        // Auto-load the last-used workouts and canvas files.
        let wp = app.workouts_path.trim();
        if !wp.is_empty() {
            app.workouts.load_from_path(expand_path(wp));
        }
        let cp = app.canvas_path.trim();
        if !cp.is_empty() {
            app.canvas_view.load_from_path(expand_path(cp));
        }
        let wadp = app.wad_path.trim();
        if !wadp.is_empty() {
            app.wad_viewer.load_from_path(expand_path(wadp));
        }

        // Honour an incoming URL fragment on wasm — `page.html#my-note`
        // opens that note after the directory has loaded.
        app.digital_garden.apply_url_fragment_if_any();

        app
    }

    /// Build the palette item list from current app state, render the
    /// palette, and dispatch any committed action.
    fn render_command_palette(&mut self, ctx: &egui::Context) {
        if !self.command_palette.open {
            return;
        }
        let mut items: Vec<PaletteItem> = Vec::new();
        // Windows — every togglable surface in the app.
        for w in [
            WindowKind::DigitalGarden,
            WindowKind::About,
            WindowKind::Resume,
            WindowKind::Projects,
            WindowKind::Calculator,
            WindowKind::BinaryClock,
            WindowKind::Canvas,
            WindowKind::Workouts,
            WindowKind::Collections,
            WindowKind::Bezier,
            WindowKind::PaletteStudio,
            WindowKind::Raycaster,
            WindowKind::TimestampConverter,
            WindowKind::WadViewer,
            WindowKind::AppEvents,
        ] {
            items.push(PaletteItem::Window(w));
        }
        // System actions.
        for s in [
            SystemAction::OrganizeWindows,
            SystemAction::ZoomIn,
            SystemAction::ZoomOut,
            SystemAction::ResetZoom,
        ] {
            items.push(PaletteItem::System(s));
        }
        // Color schemes — flip the active Poline-driven accent. Listed
        // by `Scheme::ALL` so adding new presets doesn't require a
        // matching change here.
        for s in crate::palette::Scheme::ALL {
            items.push(PaletteItem::Scheme(*s));
        }
        // Notes — only when a directory is loaded. Recent notes go first
        // (newest-first, deduped) so an empty-query Cmd+K → Enter jumps
        // back to the last note. The palette's stable sort by score
        // preserves this ordering on ties; for non-empty queries, recents
        // simply get a small ranking nudge over equally-scored matches.
        if let Some(dir) = self.digital_garden.note_directory.as_ref() {
            let recent_ids: Vec<String> = self.digital_garden.recent_notes(5);
            let mut seen: std::collections::HashSet<String> = Default::default();
            for id in &recent_ids {
                if let Some(note) = dir.get_note(id) {
                    seen.insert(id.clone());
                    items.push(PaletteItem::Note {
                        id: note.id.clone(),
                        title: note.title(),
                    });
                }
            }
            for note in dir.published_notes() {
                if seen.contains(&note.id) {
                    continue;
                }
                items.push(PaletteItem::Note {
                    id: note.id.clone(),
                    title: note.title(),
                });
            }
        }

        let Some(result) = self.command_palette.ui(ctx, items) else {
            return;
        };
        match result {
            PaletteResult::OpenWindow(kind) => self.set_window_open(kind),
            PaletteResult::OpenNote(id) => {
                if self.digital_garden.load_note(&id).is_some() {
                    self.digital_garden_is_open = true;
                }
            }
            PaletteResult::System(action) => match action {
                SystemAction::OrganizeWindows => {
                    ctx.memory_mut(|mem| mem.reset_areas());
                }
                SystemAction::ZoomIn => egui::gui_zoom::zoom_in(ctx),
                SystemAction::ZoomOut => egui::gui_zoom::zoom_out(ctx),
                SystemAction::ResetZoom => ctx.set_zoom_factor(1.0),
            },
            PaletteResult::Scheme(scheme) => {
                crate::palette::set_active_scheme(scheme);
                // Mirror to the persisted index so the next launch
                // restores this scheme.
                self.palette_scheme_idx = scheme.to_index();
                ctx.request_repaint();
            }
        }
    }

    fn set_window_open(&mut self, kind: WindowKind) {
        *self.window_flag_mut(kind) = true;
    }

    /// Toggle a window between open/closed. Driven by the Cmd+N
    /// keyboard shortcuts so a second press dismisses the window.
    fn toggle_window_open(&mut self, kind: WindowKind) {
        let flag = self.window_flag_mut(kind);
        *flag = !*flag;
    }

    /// Render one sidebar row: a `selectable_label` whose highlight
    /// reflects whether the window is open, and whose click toggles it.
    /// Pulls the open-state through `WindowKind` so the sidebar, palette,
    /// and Cmd+N shortcuts share a single dispatch.
    fn sidebar_toggle(&mut self, ui: &mut Ui, kind: WindowKind, label: &str) {
        let is_open = *self.window_flag_mut(kind);
        if ui.selectable_label(is_open, label).clicked() {
            self.toggle_window_open(kind);
        }
    }

    fn window_flag_mut(&mut self, kind: WindowKind) -> &mut bool {
        match kind {
            WindowKind::About => &mut self.about_is_open,
            WindowKind::Calculator => &mut self.calc_is_open,
            WindowKind::Resume => &mut self.resume_is_open,
            WindowKind::DigitalGarden => &mut self.digital_garden_is_open,
            WindowKind::Projects => &mut self.projects_is_open,
            WindowKind::Canvas => &mut self.canvas_is_open,
            WindowKind::Workouts => &mut self.workouts_is_open,
            WindowKind::Collections => &mut self.collections_is_open,
            WindowKind::BinaryClock => &mut self.binary_clock_is_open,
            WindowKind::Bezier => &mut self.bezier_is_open,
            WindowKind::PaletteStudio => &mut self.palette_studio_is_open,
            WindowKind::Raycaster => &mut self.raycaster_is_open,
            WindowKind::TimestampConverter => &mut self.timestamp_converter_is_open,
            WindowKind::WadViewer => &mut self.wad_viewer_is_open,
            WindowKind::AppEvents => &mut self.events_is_open,
        }
    }

    /// Render the confirmation modal for any in-flight destructive op.
    /// Modal blocks input behind it; Esc/click-outside dismisses (cancel),
    /// the right-aligned confirm button executes the action.
    fn render_pending_confirm(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.pending_confirm else {
            return;
        };
        let (heading, body, confirm_label) = match pending {
            PendingConfirm::ChangeNotesFolder => (
                "Change notes folder?",
                "This drops the loaded directory, browse history, and graph state. The notes themselves are not modified.",
                "Change folder",
            ),
            PendingConfirm::ResetEguiMemory => (
                "Reset egui memory?",
                "This clears scroll positions, window placements, and other per-widget UI state. Your data is not affected.",
                "Reset memory",
            ),
        };
        let response = egui::Modal::new(egui::Id::new("pending_confirm")).show(ctx, |ui| {
            ui.set_width(360.0);
            ui.heading(heading);
            ui.add_space(6.0);
            ui.label(body);
            ui.add_space(12.0);
            let (_, right) = egui::Sides::new().show(
                ui,
                |_| {},
                |ui| {
                    let confirmed = ui
                        .add(egui::Button::new(confirm_label).fill(ui.visuals().error_fg_color))
                        .clicked();
                    let cancelled = ui.button("Cancel").clicked();
                    (confirmed, cancelled)
                },
            );
            right
        });
        let (confirmed, cancelled) = response.inner;
        if confirmed {
            match pending {
                PendingConfirm::ChangeNotesFolder => self.digital_garden.close_directory(),
                PendingConfirm::ResetEguiMemory => {
                    ctx.memory_mut(|mem| *mem = Default::default());
                }
            }
            self.pending_confirm = None;
        } else if cancelled || response.should_close() {
            self.pending_confirm = None;
        }
    }
}

impl eframe::App for TemplateApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `Panel`, `Window` or `Area`.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // egui 0.34: `Ui` derefs to `Context`, but we still need a clonable
        // handle for `Window::show(ctx, ...)` further down.
        let ctx = ui.ctx().clone();

        // Cmd-K / Ctrl-K toggles the command palette. `consume_shortcut`
        // checks atomically and clears the press so other handlers don't
        // also fire on the same keystroke.
        if ctx.input_mut(|i| i.consume_shortcut(&TOGGLE_SHORTCUT)) {
            if self.command_palette.open {
                self.command_palette.close();
            } else {
                self.command_palette.open();
            }
        }
        // Cmd+1..5 toggle the most-used windows. Same consume-and-clear
        // pattern as the palette shortcut.
        for (shortcut, kind) in WINDOW_SHORTCUTS {
            if ctx.input_mut(|i| i.consume_shortcut(shortcut)) {
                self.toggle_window_open(*kind);
            }
        }
        // Re-stamp the theme on every frame so the whole app chrome — top
        // panel, outer sidebar, window frames — picks up the named palette
        // (and any runtime Poline tinting) regardless of which window the
        // user is currently in. Previously the theme was only applied at
        // startup + on Settings changes, so named themes only reached the
        // Digital Garden's interior.
        self.digital_garden.apply_theme(&ctx);

        ctx.output(|o| {
            for event in &o.events {
                self.output_event_history.push_back(event.clone());
            }
        });
        while self.output_event_history.len() > 1000 {
            self.output_event_history.pop_front();
        }
        egui::Panel::top("top_panel").show_inside(ui, |ui| {
            // We own the visuals via `DigitalGarden::apply_theme`, so the
            // built-in egui light/dark switch would just fight us. Named
            // themes (Garden Dark/Light, Obsidian Dark/Light) live in the
            // Digital Garden's Settings modal instead.
            egui::MenuBar::new().ui(ui, |ui| {
                file_menu_button(ui, _frame, &mut self.pending_confirm);
            });
        });

        if !is_mobile(&ctx) {
            egui::Panel::left("side_panel")
                .default_size(250.0)
                .resizable(false)
                .show_inside(ui, |ui| {
                    let accent = crate::palette::accent_now();
                    ui.heading("🔧 Garden Tools");
                    ui.separator();
                    ui.hyperlink_to("my personal github", "https://github.com/vaporeyes");
                    ui.hyperlink_to("my blog", "https://josh.contact");
                    egui::warn_if_debug_build(ui);

                    // `selectable_label` toggle-highlights when its flag is
                    // true, so the sidebar shows at a glance which windows
                    // are open. Clicking now toggles: a second click on a
                    // highlighted label closes the window. Window's X
                    // button also still works.
                    sidebar_section(ui, "identity", accent);
                    self.sidebar_toggle(ui, WindowKind::About, "About Me");
                    self.sidebar_toggle(ui, WindowKind::Resume, "Pseudo-Resumé");
                    self.sidebar_toggle(ui, WindowKind::Projects, "Projects");

                    sidebar_section(ui, "tools", accent);
                    self.sidebar_toggle(ui, WindowKind::Calculator, "Calculator");
                    self.sidebar_toggle(ui, WindowKind::BinaryClock, "Binary Clock");
                    self.sidebar_toggle(ui, WindowKind::Canvas, "Canvas");
                    self.sidebar_toggle(ui, WindowKind::Workouts, "Workouts");
                    self.sidebar_toggle(ui, WindowKind::Collections, "Collections");
                    self.sidebar_toggle(ui, WindowKind::Bezier, "Bezier");
                    self.sidebar_toggle(ui, WindowKind::PaletteStudio, "Palette Studio");
                    self.sidebar_toggle(ui, WindowKind::Raycaster, "Raycaster");
                    self.sidebar_toggle(ui, WindowKind::TimestampConverter, "Timestamp Converter");
                    self.sidebar_toggle(ui, WindowKind::WadViewer, "WAD Viewer");

                    sidebar_section(ui, "notes", accent);
                    self.sidebar_toggle(ui, WindowKind::DigitalGarden, "Digital Garden");
                    if self.digital_garden.note_directory.is_some()
                        && ui.button("Change notes folder").clicked()
                    {
                        // Confirm before dropping the directory; close_directory
                        // releases the filesystem watcher and clears the
                        // history/graph state, none of which is recoverable.
                        self.pending_confirm = Some(PendingConfirm::ChangeNotesFolder);
                    }

                    sidebar_section(ui, "system", accent);
                    self.sidebar_toggle(ui, WindowKind::AppEvents, "App Events");
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            ui.label("powered by ");
                            ui.hyperlink_to("egui", "https://github.com/emilk/egui");
                            ui.label(" and ");
                            ui.hyperlink_to(
                                "eframe",
                                "https://github.com/emilk/egui/tree/master/eframe",
                            );
                        });
                    });
                });
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
            // 4-corner mesh gradient drawn behind the fractal clock and
            // the "My Digital Garden" heading. Color anchors come from
            // `palette::anchors_for_now`, so the backdrop drifts through
            // the day in step with the rest of the named-theme system.
            paint_ambient_gradient(ui);
            self.fractal_clock.ui(ui, Some(seconds_since_midnight()));
            ui.vertical_centered(|ui| {
                ui.heading("🏡 My Digital Garden");
            });
        });

        egui::Window::new("A Calculator")
            .open(&mut self.calc_is_open)
            .show(&ctx, |ui| self.calculator.ui(ui));

        egui::Window::new("Binary Clock")
            .open(&mut self.binary_clock_is_open)
            .default_width(360.0)
            .default_height(260.0)
            .show(&ctx, |ui| {
                self.binary_clock.ui(ui, Some(seconds_since_midnight()))
            });

        egui::Window::new("Pseudo-Resumé")
            .open(&mut self.resume_is_open)
            .fixed_size([760.0, 760.0])
            .show(&ctx, |ui| easy_mark(ui, EASYMARK_DATA));

        egui::Window::new("About Me")
            .open(&mut self.about_is_open)
            .default_width(600.0)
            .default_height(640.0)
            .show(&ctx, |ui| self.about_me.ui(ui));

        egui::Window::new("Projects")
            .open(&mut self.projects_is_open)
            .default_width(620.0)
            .default_height(640.0)
            .show(&ctx, |ui| self.projects.ui(ui));

        let canvas_clicked_file = egui::Window::new("Canvas")
            .open(&mut self.canvas_is_open)
            .default_width(900.0)
            .default_height(640.0)
            .show(&ctx, |ui| {
                // Pass through the currently-loaded notes directory so
                // markdown rendered inside text nodes can resolve
                // `[[wiki-links]]` across to the Digital Garden.
                let directory = self.digital_garden.note_directory.as_ref();
                self.canvas_view.ui(ui, directory)
            })
            .and_then(|r| r.inner)
            .flatten();
        // A file-type canvas node click = "go open this note". Resolve it
        // against the Digital Garden and, if found, surface the garden.
        if let Some(note_id) = canvas_clicked_file {
            if self.digital_garden.load_note(&note_id).is_some() {
                self.digital_garden_is_open = true;
            }
        }
        // Keep the persisted canvas path in sync with whatever the view
        // most recently loaded, so next launch auto-restores it.
        if let Some(p) = self.canvas_view.loaded_path() {
            let s = p.to_string_lossy().to_string();
            if s != self.canvas_path {
                self.canvas_path = s;
            }
        }

        egui::Window::new("Workouts")
            .open(&mut self.workouts_is_open)
            .default_width(900.0)
            .default_height(420.0)
            .show(&ctx, |ui| self.workouts.ui(ui));

        egui::Window::new("Collections")
            .open(&mut self.collections_is_open)
            .default_width(720.0)
            .default_height(640.0)
            .show(&ctx, |ui| self.collections.ui(ui));

        egui::Window::new("Bezier Playground")
            .open(&mut self.bezier_is_open)
            .default_width(560.0)
            .default_height(520.0)
            .show(&ctx, |ui| self.bezier.ui(ui));

        egui::Window::new("Palette Studio")
            .open(&mut self.palette_studio_is_open)
            .default_width(640.0)
            .default_height(520.0)
            .show(&ctx, |ui| self.palette_studio.ui(ui));

        egui::Window::new("Raycaster")
            .open(&mut self.raycaster_is_open)
            .default_width(720.0)
            .default_height(520.0)
            .min_width(360.0)
            .min_height(280.0)
            .show(&ctx, |ui| self.raycaster.ui(ui));

        egui::Window::new("Timestamp Converter")
            .open(&mut self.timestamp_converter_is_open)
            .default_width(560.0)
            .default_height(360.0)
            .min_width(420.0)
            .show(&ctx, |ui| self.timestamp_converter.ui(ui));

        egui::Window::new("WAD Viewer")
            .open(&mut self.wad_viewer_is_open)
            .default_width(900.0)
            .default_height(640.0)
            .min_width(500.0)
            .min_height(360.0)
            .show(&ctx, |ui| self.wad_viewer.ui(ui));
        // Mirror the loaded WAD path into the persisted field so the
        // next launch auto-rehydrates the same file.
        if let Some(p) = self.wad_viewer.loaded_path() {
            let s = p.to_string_lossy().to_string();
            if s != self.wad_path {
                self.wad_path = s;
            }
        }
        if let Some(p) = self.workouts.loaded_path() {
            let s = p.to_string_lossy().to_string();
            if s != self.workouts_path {
                self.workouts_path = s;
            }
        }

        egui::Window::new("Digital Garden")
            .open(&mut self.digital_garden_is_open)
            .resizable(true)
            .default_width(1080.0)
            .default_height(720.0)
            .min_width(640.0)
            .min_height(420.0)
            .show(&ctx, |ui| {
                // If notes directory is not set, show a welcome panel with
                // path controls and example paths.
                if self.digital_garden.note_directory.is_none() {
                    welcome_screen(
                        ui,
                        &mut self.notes_directory_path,
                        &mut self.notes_directory_error,
                        &mut self.digital_garden,
                    );
                } else {
                    // Render the digital garden *inside* the window's ui so its
                    // panels stay scoped to the floating window rather than
                    // leaking out to the root context.
                    self.digital_garden.ui(ui);
                }
            });

        egui::Window::new("📤 Output Events")
            .open(&mut self.events_is_open)
            .resizable(true)
            .default_width(520.0)
            .show(&ctx, |ui| {
                clock_button(ui, seconds_since_midnight());
                ui.label("Recent output events from egui.");

                ui.separator();

                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for event in &self.output_event_history {
                            ui.label(format!("{:?}", event));
                        }
                    });
            });

        self.render_pending_confirm(&ctx);
        self.render_command_palette(&ctx);
    }


    fn on_exit(&mut self) {}

    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(30)
    }


    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // NOTE: a bright gray makes the shadows of the windows look weird.
        // We use a bit of transparency so that if the user switches on the
        // `transparent()` option they get immediate results.
        let c = egui::Color32::from_rgba_unmultiplied(12, 12, 12, 180);
        let [r, g, b, a] = c.to_array();
        [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0]

        // _visuals.window_fill() would also be a natural choice
    }

    fn persist_egui_memory(&self) -> bool {
        true
    }
}

fn seconds_since_midnight() -> f64 {
    use chrono::Timelike;
    let time = chrono::Local::now().time();
    time.num_seconds_from_midnight() as f64 + 1e-9 * (time.nanosecond() as f64)
}

// egui 0.32 unified menus on top of the new Popup API; menus now close on
// click by default, so explicit `ui.close_menu()` calls are obsolete.
// egui 0.32 unified menus on top of the new Popup API; menus now close on
// click by default, so explicit `ui.close_menu()` calls are obsolete.
#[cfg(target_arch = "wasm32")]
fn file_menu_button(
    ui: &mut Ui,
    _frame: &mut eframe::Frame,
    pending_confirm: &mut Option<PendingConfirm>,
) {
    ui.menu_button("File", |ui| {
        egui::gui_zoom::zoom_menu_buttons(ui);
        ui.separator();
        if ui
            .button("Reset egui memory")
            .on_hover_text("Forget scroll, positions, sizes etc")
            .clicked()
        {
            *pending_confirm = Some(PendingConfirm::ResetEguiMemory);
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn file_menu_button(
    ui: &mut Ui,
    _frame: &mut eframe::Frame,
    pending_confirm: &mut Option<PendingConfirm>,
) {
    ui.menu_button("File", |ui| {
        egui::gui_zoom::zoom_menu_buttons(ui);
        ui.separator();
        if ui.button("Organize windows").clicked() {
            ui.ctx().memory_mut(|mem| mem.reset_areas());
        }
        if ui
            .button("Reset egui memory")
            .on_hover_text("Forget scroll, positions, sizes etc")
            .clicked()
        {
            *pending_confirm = Some(PendingConfirm::ResetEguiMemory);
        }
        if ui.button("Quit").clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    });
}

/// Register the bundled OFL fonts and place them at the front of each
/// family list. We keep the egui defaults around as fallbacks for
/// glyphs our chosen fonts don't cover (most notably emoji, which
/// `epaint_default_fonts` ships via NotoEmoji).
///
/// Font slots:
/// * `FontFamily::Proportional` → Atkinson Hyperlegible (UI chrome,
///   sidebar, buttons)
/// * `FontFamily::Name("Serif")` → Lora (markdown body + headings)
/// * `FontFamily::Monospace` → IBM Plex Mono (code blocks, inline
///   code, the calculator readout, the binary clock numerals)
fn install_fonts(ctx: &egui::Context) {
    use std::sync::Arc;

    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "atkinson".to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/AtkinsonHyperlegible-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        "lora".to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/Lora-Variable.ttf"
        ))),
    );
    fonts.font_data.insert(
        "plex_mono".to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/IBMPlexMono-Regular.ttf"
        ))),
    );

    // Insert at the front of each family so our font is preferred but
    // egui's bundled fonts still resolve glyphs ours don't have.
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "atkinson".to_owned());
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "plex_mono".to_owned());
    // Custom serif family — used by `markdown_parser` for body text and
    // headings via `FontId::new(size, FontFamily::Name("Serif".into()))`.
    fonts.families.insert(
        egui::FontFamily::Name("Serif".into()),
        vec![
            "lora".to_owned(),
            "atkinson".to_owned(),
        ],
    );

    ctx.set_fonts(fonts);
}

/// Paint a 4-corner mesh gradient that fills the central panel. Each
/// corner gets a heavily darkened tint from one of the time-of-day
/// anchors so the backdrop reads as ambient depth, not a flat fill.
fn paint_ambient_gradient(ui: &mut egui::Ui) {
    let rect = ui.max_rect();
    let (a, b) = crate::palette::anchors_for_now();
    let a = a.to_color32();
    let b = b.to_color32();
    // Mute hard so the gradient sits *behind* foreground content. Without
    // this the heading and fractal clock get washed by the saturation.
    let dim = |c: egui::Color32, k: f32| -> egui::Color32 {
        egui::Color32::from_rgba_unmultiplied(
            ((c.r() as f32) * k) as u8,
            ((c.g() as f32) * k) as u8,
            ((c.b() as f32) * k) as u8,
            255,
        )
    };
    let tl = dim(a, 0.18);
    let tr = dim(b, 0.12);
    let bl = dim(b, 0.10);
    let br = dim(a, 0.22);

    let mut mesh = egui::epaint::Mesh::default();
    mesh.colored_vertex(rect.left_top(), tl);
    mesh.colored_vertex(rect.right_top(), tr);
    mesh.colored_vertex(rect.right_bottom(), br);
    mesh.colored_vertex(rect.left_bottom(), bl);
    // Two triangles forming the quad (CCW): TL-TR-BR, TL-BR-BL.
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    ui.painter().add(egui::Shape::mesh(mesh));
}

pub fn is_mobile(ctx: &egui::Context) -> bool {
    let screen_size = ctx.input(|i| i.content_rect().size());
    screen_size.x < 550.0
}

/// The pre-directory welcome screen for the Digital Garden window.
/// Offers a path input, file picker, "use astro-blog posts" shortcut, a
/// handful of example paths the user can click to prefill, and a subtle
/// palette-tinted backdrop so it feels less like a blank form.
fn welcome_screen(
    ui: &mut Ui,
    path: &mut String,
    error: &mut Option<String>,
    digital_garden: &mut DigitalGarden,
) {
    let accent = crate::palette::accent_now();

    // Subtle accent-tinted backdrop for the whole panel. We paint this
    // before the frame content so existing widgets sit on top.
    let full = ui.max_rect();
    ui.painter().rect_filled(full, 0.0, accent.linear_multiply(0.04));

    ui.add_space(20.0);
    ui.vertical_centered(|ui| {
        ui.set_max_width(520.0);
        ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
            ui.label(
                egui::RichText::new("Welcome to Digital Garden")
                    .size(28.0)
                    .strong()
                    .color(accent),
            );
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new("Point it at any folder of `.md` notes with YAML frontmatter.")
                    .weak(),
            );

            ui.add_space(16.0);
            ui.label(egui::RichText::new("PATH").small().strong().color(accent));
            ui.add_space(2.0);
            ui.add(
                egui::TextEdit::singleline(path)
                    .desired_width(f32::INFINITY)
                    .hint_text("/absolute/path or ~/relative/to/home"),
            );

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button("Load").clicked() {
                    try_set_notes_dir(digital_garden, error, expand_path(path.trim()), path);
                }
                #[cfg(not(target_arch = "wasm32"))]
                if ui.button("📁 Browse…").clicked() {
                    if let Some(picked) = rfd::FileDialog::new()
                        .set_title("Choose a notes directory")
                        .pick_folder()
                    {
                        *path = picked.to_string_lossy().to_string();
                        try_set_notes_dir(digital_garden, error, picked, path);
                    }
                }
                if ui.button("Use astro-blog posts").clicked() {
                    *path = "~/dev/projects/astro-blog/src/content/posts".to_string();
                    try_set_notes_dir(digital_garden, error, expand_path(path.trim()), path);
                }
            });

            if let Some(err) = error.as_deref() {
                ui.add_space(6.0);
                ui.colored_label(egui::Color32::from_rgb(220, 80, 80), err);
            }

            ui.add_space(22.0);
            ui.label(egui::RichText::new("EXAMPLES").small().strong().color(accent));
            ui.add_space(4.0);
            let examples: &[&str] = &[
                "~/Documents/notes",
                "~/Obsidian/my-vault",
                "~/dev/projects/astro-blog/src/content/posts",
                "./notes",
            ];
            for ex in examples {
                let resp = ui
                    .add(
                        egui::Label::new(
                            egui::RichText::new(format!("  ↳ {}", ex)).monospace().weak(),
                        )
                        .sense(egui::Sense::click()),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text("Click to prefill the path field");
                if resp.clicked() {
                    *path = ex.to_string();
                }
            }

            ui.add_space(20.0);
            ui.label(
                egui::RichText::new(
                    "Astro's `pubDate` frontmatter is recognized alongside `created`.",
                )
                .small()
                .weak(),
            );
        });
    });
}

fn try_set_notes_dir(
    dg: &mut DigitalGarden,
    error: &mut Option<String>,
    resolved: PathBuf,
    _display_path: &str,
) {
    match dg.set_notes_directory(&resolved) {
        Ok(()) => *error = None,
        Err(err) => {
            *error = Some(format!("Couldn't load {}: {}", resolved.display(), err));
        }
    }
}

/// Small-caps accent header used between button groups in the outer sidebar.
fn sidebar_section(ui: &mut Ui, label: &str, accent: egui::Color32) {
    ui.add_space(10.0);
    ui.label(
        egui::RichText::new(label.to_uppercase())
            .small()
            .strong()
            .color(accent),
    );
    ui.add_space(2.0);
}

/// Minimal `~/` expansion so users can enter paths the same way they'd type them
/// in a shell. Avoids pulling in `shellexpand` / `dirs` for a single prefix.
fn expand_path(s: &str) -> PathBuf {
    if let Some(stripped) = s.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(stripped);
        }
    }
    PathBuf::from(s)
}

const EASYMARK_DATA: &str = r#"
# Digital Garden

I saw the idea of a digital garden and it intrigued me, so I
decided to try building one with egui and Rust. This is the result 😁

## About Me

I'm an SRE / Backend / Platform engineer based in Middle Tennessee.
My day job is infrastructure; the rest of the time I'm building
weird apps, lifting heavy things, and making stuff — usually
somewhere between the terminal and the rack. Current tool belt
leans Python, Go and TypeScript, with ongoing tinkering in Rust,
Elixir and Swift.

## Pseudo-Resumé

- `2008-2011` - *DTS America*: I started out in the private sector in
2008 as a helpdesk associate and moved to system administrator
shortly after. Lots of network engineering as well.
- `2011-2016` - *Centerstone*: Non-profit as the senior system
administrator. A lot of VMware and virtualization on-prem, then.
- `2016-2017` - *BNY Mellon*: Contract work with Powershell and Cisco
UCS
- `2017-2018` - *Ingram Content Group*: Linux engineer with the book
group maintaining the core Linux infrastructure
- `2018-2018` - *NASBA*: Systems engineer architecting medium-sized apps
until the IT department was outsourced suddenly
- `2018-2019` - *Eventbrite*: Site-reliability engineer with the platform
team responsible for several types of workloads in the AWS cloud
- `2019-present`: *XOi Technologies*: Senior platform engineer with duties
primarily in AWS for mobile, web and backend applications

"#;
