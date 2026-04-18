use crate::apps::clock_button;
use crate::apps::easy_mark;
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
            calculator: Default::default(),
            fractal_clock: Default::default(),
            binary_clock: Default::default(),
            about_me: Default::default(),
            projects: Default::default(),
            canvas_view: Default::default(),
            workouts: Default::default(),
            digital_garden: DigitalGarden::default(),
            notes_directory_path: String::new(),
            workouts_path: String::new(),
            canvas_path: String::new(),
            notes_directory_error: None,
            output_event_history: Default::default(),
        }
    }
}

impl TemplateApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customized the look at feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        let mut app: Self = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        };

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

        // Honour an incoming URL fragment on wasm — `page.html#my-note`
        // opens that note after the directory has loaded.
        app.digital_garden.apply_url_fragment_if_any();

        app
    }
}

impl eframe::App for TemplateApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Re-stamp the theme on every frame so the whole app chrome — top
        // panel, outer sidebar, window frames — picks up the named palette
        // (and any runtime Poline tinting) regardless of which window the
        // user is currently in. Previously the theme was only applied at
        // startup + on Settings changes, so named themes only reached the
        // Digital Garden's interior.
        self.digital_garden.apply_theme(ctx);

        ctx.output(|o| {
            for event in &o.events {
                self.output_event_history.push_back(event.clone());
            }
        });
        while self.output_event_history.len() > 1000 {
            self.output_event_history.pop_front();
        }
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // We own the visuals via `DigitalGarden::apply_theme`, so the
            // built-in egui light/dark switch would just fight us. Named
            // themes (Garden Dark/Light, Obsidian Dark/Light) live in the
            // Digital Garden's Settings modal instead.
            egui::menu::bar(ui, |ui| {
                file_menu_button(ui, _frame);
            });
        });

        if is_mobile(ctx) == false {
            egui::SidePanel::left("side_panel")
                .default_width(250.0)
                .resizable(false)
                .show(ctx, |ui| {
                    let accent = crate::palette::accent_now();
                    ui.heading("🔧 Garden Tools");
                    ui.separator();
                    ui.hyperlink_to("my personal github", "https://github.com/vaporeyes");
                    ui.hyperlink_to("my blog", "https://josh.contact");
                    egui::warn_if_debug_build(ui);

                    // `selectable_label` toggle-highlights when its flag is
                    // true, so the sidebar can show at a glance which
                    // windows are currently open. Clicking still opens the
                    // window; we don't wire a close action here so the
                    // user can still hit the window's X to close.
                    sidebar_section(ui, "identity", accent);
                    if ui.selectable_label(self.about_is_open, "About Me").clicked() {
                        self.about_is_open = true;
                    }
                    if ui
                        .selectable_label(self.resume_is_open, "Pseudo-Resumé")
                        .clicked()
                    {
                        self.resume_is_open = true;
                    }
                    if ui.selectable_label(self.projects_is_open, "Projects").clicked() {
                        self.projects_is_open = true;
                    }

                    sidebar_section(ui, "tools", accent);
                    if ui.selectable_label(self.calc_is_open, "Calculator").clicked() {
                        self.calc_is_open = true;
                    }
                    if ui
                        .selectable_label(self.binary_clock_is_open, "Binary Clock")
                        .clicked()
                    {
                        self.binary_clock_is_open = true;
                    }
                    if ui.selectable_label(self.canvas_is_open, "Canvas").clicked() {
                        self.canvas_is_open = true;
                    }
                    if ui
                        .selectable_label(self.workouts_is_open, "Workouts")
                        .clicked()
                    {
                        self.workouts_is_open = true;
                    }

                    sidebar_section(ui, "notes", accent);
                    if ui
                        .selectable_label(self.digital_garden_is_open, "Digital Garden")
                        .clicked()
                    {
                        self.digital_garden_is_open = true;
                    }
                    if self.digital_garden.note_directory.is_some()
                        && ui.button("Change notes folder").clicked()
                    {
                        // Drop everything tied to the old path — including
                        // the filesystem watcher, which used to leak.
                        self.digital_garden.close_directory();
                    }

                    sidebar_section(ui, "system", accent);
                    if ui.selectable_label(self.events_is_open, "App Events").clicked() {
                        self.events_is_open = true;
                    }
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

        egui::CentralPanel::default().show(ctx, |ui| {
            self.fractal_clock.ui(ui, Some(seconds_since_midnight()));
            ui.vertical_centered(|ui| {
                ui.heading("🏡 My Digital Garden");
            });
        });

        egui::Window::new("A Calculator")
            .open(&mut self.calc_is_open)
            .show(ctx, |ui| self.calculator.ui(ui));

        egui::Window::new("Binary Clock")
            .open(&mut self.binary_clock_is_open)
            .default_width(360.0)
            .default_height(260.0)
            .show(ctx, |ui| {
                self.binary_clock.ui(ui, Some(seconds_since_midnight()))
            });

        egui::Window::new("Pseudo-Resumé")
            .open(&mut self.resume_is_open)
            .fixed_size([760.0, 760.0])
            .show(ctx, |ui| easy_mark(ui, EASYMARK_DATA));

        egui::Window::new("About Me")
            .open(&mut self.about_is_open)
            .default_width(600.0)
            .default_height(640.0)
            .show(ctx, |ui| self.about_me.ui(ui));

        egui::Window::new("Projects")
            .open(&mut self.projects_is_open)
            .default_width(620.0)
            .default_height(640.0)
            .show(ctx, |ui| self.projects.ui(ui));

        let canvas_clicked_file = egui::Window::new("Canvas")
            .open(&mut self.canvas_is_open)
            .default_width(900.0)
            .default_height(640.0)
            .show(ctx, |ui| {
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
            .show(ctx, |ui| self.workouts.ui(ui));
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
            .show(ctx, |ui| {
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
            .show(ctx, |ui| {
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
    }


    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {}

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

#[cfg(target_arch = "wasm32")]
fn file_menu_button(ui: &mut Ui, _frame: &mut eframe::Frame) {
    ui.menu_button("File", |ui| {
        if ui
            .button("Reset egui memory")
            .on_hover_text("Forget scroll, positions, sizes etc")
            .clicked()
        {
            ui.ctx().memory_mut(|mem| *mem = Default::default());
            ui.close_menu();
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn file_menu_button(ui: &mut Ui, _frame: &mut eframe::Frame) {
    ui.menu_button("File", |ui| {
        if ui.button("Organize windows").clicked() {
            ui.ctx().memory_mut(|mem| mem.reset_areas());
            ui.close_menu();
        }
        if ui
            .button("Reset egui memory")
            .on_hover_text("Forget scroll, positions, sizes etc")
            .clicked()
        {
            ui.ctx().memory_mut(|mem| *mem = Default::default());
            ui.close_menu();
        }
        if ui.button("Quit").clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    });
}

pub fn is_mobile(ctx: &egui::Context) -> bool {
    let screen_size = ctx.input(|i| i.screen_rect().size());
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
