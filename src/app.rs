use egui::Ui;
/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct TemplateApp {
    // Example stuff:
    label: String,

    // this how you opt-out of serialization of a member
    #[serde(skip)]
    value: f32,
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            // Example stuff:
            label: "Hello World!".to_owned(),
            value: 2.7,
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
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
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
        let Self { label: _, value: _ } = self;

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:
            egui::menu::bar(ui, |ui| {
                egui::widgets::global_dark_light_mode_switch(ui);
                ui.separator();
                file_menu_button(ui, _frame);
            });
        });

        egui::SidePanel::left("side_panel")
            .default_width(250.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading("🔧 Garden Tools");
                ui.separator();
                ui.hyperlink("https://github.com/vaporeyes");
                egui::warn_if_debug_build(ui);

                ui.separator();
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

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("🏡 My Digital Garden");
        });

        egui::Window::new("About Me")
            .open(&mut true)
            .show(ctx, |ui| {
                ui.label("Life Skills");
                egui::Grid::new("life_skills")
                    .num_columns(2)
                    .spacing([40.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.add(doc_link_label("Drawing & Painting", "DrawingAndPainting"));
                        let progress = 300.0 / 360.0;
                        let progress_bar = egui::ProgressBar::new(progress).show_percentage();
                        ui.add(progress_bar);
                        ui.end_row();
                        ui.add(doc_link_label("Cooking", "Cooking"));
                        let progress = 290.0 / 360.0;
                        let progress_bar = egui::ProgressBar::new(progress).show_percentage();
                        ui.add(progress_bar);
                        ui.end_row();
                        ui.add(doc_link_label("Model Building", "ModelBuilding"));
                        let progress = 200.0 / 360.0;
                        let progress_bar = egui::ProgressBar::new(progress).show_percentage();
                        ui.add(progress_bar);
                        ui.end_row();
                    });
                ui.separator();
                ui.label("Work Skills");
                egui::Grid::new("work_skills")
                    .num_columns(2)
                    .spacing([100.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.add(doc_link_label("Python", "https://google.com"));
                        let progress = 234.0 / 360.0;
                        let progress_bar = egui::ProgressBar::new(progress).show_percentage();
                        ui.add(progress_bar);
                        ui.end_row();
                        ui.add(doc_link_label("Javascript", "js_programming"));
                        let progress = 126.0 / 360.0;
                        let progress_bar = egui::ProgressBar::new(progress).show_percentage();
                        ui.add(progress_bar);
                        ui.end_row();
                        ui.add(doc_link_label("Rust", "rust_programming"));
                        let progress = 60.0 / 360.0;
                        let progress_bar = egui::ProgressBar::new(progress).show_percentage();
                        ui.add(progress_bar);
                        ui.end_row();
                        ui.add(doc_link_label("Elixir", "elixir_programming"));
                        let progress = 85.0 / 360.0;
                        let progress_bar = egui::ProgressBar::new(progress).show_percentage();
                        ui.add(progress_bar);
                        ui.end_row();
                    });
            });
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn on_close_event(&mut self) -> bool {
        true
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {}

    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(30)
    }

    fn max_size_points(&self) -> egui::Vec2 {
        egui::Vec2::INFINITY
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> egui::Rgba {
        // NOTE: a bright gray makes the shadows of the windows look weird.
        // We use a bit of transparency so that if the user switches on the
        // `transparent()` option they get immediate results.
        egui::Color32::from_rgba_unmultiplied(12, 12, 12, 180).into()

        // _visuals.window_fill() would also be a natural choice
    }

    fn persist_native_window(&self) -> bool {
        true
    }

    fn persist_egui_memory(&self) -> bool {
        true
    }

    fn warm_up_enabled(&self) -> bool {
        false
    }

    fn post_rendering(&mut self, _window_size_px: [u32; 2], _frame: &eframe::Frame) {}
}

#[cfg(target_arch = "wasm32")]
fn file_menu_button(ui: &mut Ui, _frame: &mut eframe::Frame) {
    ui.menu_button("File", |ui| {
        if ui
            .button("Reset egui memory")
            .on_hover_text("Forget scroll, positions, sizes etc")
            .clicked()
        {
            *ui.ctx().memory() = Default::default();
            ui.close_menu();
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn file_menu_button(ui: &mut Ui, _frame: &mut eframe::Frame) {
    ui.menu_button("File", |ui| {
        if ui.button("Organize windows").clicked() {
            ui.ctx().memory().reset_areas();
            ui.close_menu();
        }
        if ui
            .button("Reset egui memory")
            .on_hover_text("Forget scroll, positions, sizes etc")
            .clicked()
        {
            *ui.ctx().memory() = Default::default();
            ui.close_menu();
        }
        if ui.button("Quit").clicked() {
            _frame.close();
        }
    });
}

fn doc_link_label<'a>(title: &'a str, hyperlink: &'a str) -> impl egui::Widget + 'a {
    let label = format!("{}:", title);
    move |ui: &mut egui::Ui| {
        ui.hyperlink_to(label, hyperlink).on_hover_ui(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Click me!");
            });
        })
    }
}
