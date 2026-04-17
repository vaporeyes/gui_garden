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
    // Example stuff:
    label: String,

    // this how you opt-out of serialization of a member
    #[serde(skip)]
    value: f32,
    about_is_open: bool,
    calc_is_open: bool,
    clock_is_open: bool,
    events_is_open: bool,
    resume_is_open: bool,
    digital_garden_is_open: bool,
    #[serde(skip)]
    calculator: crate::apps::Calculator,
    #[serde(skip)]
    fractal_clock: crate::apps::FractalClock,
    #[serde(skip)]
    about_me: crate::about::AboutMe,
    #[serde(skip)]
    digital_garden: DigitalGarden,
    output_event_history: std::collections::VecDeque<egui::output::OutputEvent>,
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            // Example stuff:
            label: "Hello World!".to_owned(),
            value: 2.7,
            about_is_open: true,
            calc_is_open: false,
            clock_is_open: true,
            events_is_open: false,
            resume_is_open: false,
            digital_garden_is_open: false,
            calculator: Default::default(),
            fractal_clock: Default::default(),
            about_me: Default::default(),
            digital_garden: DigitalGarden::default(),
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
        ctx.output(|o| {
            for event in &o.events {
                self.output_event_history.push_back(event.clone());
            }
        });
        while self.output_event_history.len() > 1000 {
            self.output_event_history.pop_front();
        }
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:
            egui::menu::bar(ui, |ui| {
                egui::widgets::global_dark_light_mode_switch(ui);
                ui.separator();
                file_menu_button(ui, _frame);
            });
        });

        if is_mobile(ctx) == false {
            egui::SidePanel::left("side_panel")
                .default_width(250.0)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.heading("🔧 Garden Tools");
                    ui.separator();
                    ui.hyperlink_to("my personal github", "https://github.com/vaporeyes");
                    ui.separator();
                    ui.hyperlink_to("my blog", "https://josh.contact");
                    egui::warn_if_debug_build(ui);
                    ui.separator();
                    if ui.button("About Me").clicked() {
                        self.about_is_open = true;
                    }
                    if ui.button("Calculator").clicked() {
                        self.calc_is_open = true;
                    }
                    if ui.button("Pseudo-Resumé").clicked() {
                        self.resume_is_open = true;
                    }
                    if ui.button("Digital Garden").clicked() {
                        self.digital_garden_is_open = true;
                    }
                    ui.separator();
                    if ui.button("App Events").clicked() {
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

        egui::Window::new("Pseudo-Resumé")
            .open(&mut self.resume_is_open)
            .fixed_size([760.0, 760.0])
            .show(ctx, |ui| easy_mark(ui, EASYMARK_DATA));

        egui::Window::new("About Me")
            .open(&mut self.about_is_open)
            .show(ctx, |ui| self.about_me.ui(ui));

        egui::Window::new("Digital Garden")
            .open(&mut self.digital_garden_is_open)
            .resizable(true)
            .default_width(800.0)
            .default_height(600.0)
            .show(ctx, |ui| {
                // If notes directory is not set, show a file picker or allow manual path entry
                if self.digital_garden.note_directory.is_none() {
                    ui.heading("Welcome to Digital Garden");
                    ui.label("Please select a directory containing your markdown notes:");
                    
                    if ui.button("Set Example Path").clicked() {
                        // For demonstration, set to a hardcoded path (you would replace this with a file picker)
                        let example_path = PathBuf::from("./notes");
                        if let Err(err) = self.digital_garden.set_notes_directory(&example_path) {
                            eprintln!("Error setting notes directory: {}", err);
                        }
                    }
                } else {
                    // Update the digital garden UI
                    self.digital_garden.update(ctx, _frame);
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
            *ui.ctx().memory() = Default::default();
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

const EASYMARK_DATA: &str = r#"
# Digital Garden

I saw the idea of a digital garden and it intrigued me, so I
decided to try using egui and this is the result 😁

## About Me

I am currently a devops engineer in Middle Tennessee and I enjoy
tinkering with different programming languages, specifically
python, rust, javascript and elixir

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
