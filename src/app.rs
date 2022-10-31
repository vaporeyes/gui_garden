use eframe::egui;
use egui::{Ui};

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
    #[serde(skip)]
    calculator: crate::apps::Calculator,
    #[serde(skip)]
    about_me: crate::about::AboutMe,
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            // Example stuff:
            label: "Hello World!".to_owned(),
            value: 2.7,
            about_is_open: true,
            calc_is_open: false,
            calculator: Default::default(),
            about_me: Default::default()
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
                if ui.button("About Me").clicked() {
                    self.about_is_open = true;
                }
                if ui.button("Calculator").clicked() {
                    self.calc_is_open = true;
                }
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

        egui::Window::new("A Calculator")
            .open(&mut self.calc_is_open)
            .show(ctx, |ui| {
                self.calculator.ui(ui)
            });

        egui::Window::new("About Me")
            .open(&mut self.about_is_open)
            .show(ctx, |ui| {
                self.about_me.ui(ui)
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

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Calculator {
    calculator: crate::apps::Calculator,
}

impl eframe::App for Calculator {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::Window::new("A Calculator")
            .fixed_size([433.0, 433.0])
            .show(ctx, |ui| self.calculator.ui(ui));
    }
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

// fn custom_window_frame(
//     ctx: &egui::Context,
//     frame: &mut eframe::Frame,
//     title: &str,
//     add_contents: impl FnOnce(&mut egui::Ui),
// ) {
//     use egui::*;
//     let text_color = ctx.style().visuals.text_color();

//     // Height of the title bar
//     let height = 28.0;

//     egui::Area::new("a window").show(ctx, |ui| {
//         let rect = ui.max_rect();
//         let painter = ui.painter();

//         // Paint the frame:
//         painter.rect(
//             rect.shrink(1.0),
//             10.0,
//             ctx.style().visuals.window_fill(),
//             Stroke::new(1.0, text_color),
//         );

//         // Paint the title:
//         painter.text(
//             rect.center_top() + vec2(0.0, height / 2.0),
//             Align2::CENTER_CENTER,
//             title,
//             FontId::proportional(height * 0.8),
//             text_color,
//         );

//         // Paint the line under the title:
//         painter.line_segment(
//             [
//                 rect.left_top() + vec2(2.0, height),
//                 rect.right_top() + vec2(-2.0, height),
//             ],
//             Stroke::new(1.0, text_color),
//         );

//         // Add the close button:
//         let close_response = ui.put(
//             Rect::from_min_size(rect.left_top(), Vec2::splat(height)),
//             Button::new(RichText::new("❌").size(height - 4.0)).frame(false),
//         );
//         if close_response.clicked() {
//             frame.close();
//         }

//         // Interact with the title bar (drag to move window):
//         let title_bar_rect = {
//             let mut rect = rect;
//             rect.max.y = rect.min.y + height;
//             rect
//         };
//         let title_bar_response = ui.interact(title_bar_rect, Id::new("title_bar"), Sense::click());
//         if title_bar_response.is_pointer_button_down_on() {
//             frame.drag_window();
//         }

//         // Add the contents:
//         let content_rect = {
//             let mut rect = rect;
//             rect.min.y = title_bar_rect.max.y;
//             rect
//         }
//         .shrink(4.0);
//         let mut content_ui = ui.child_ui(content_rect, *ui.layout());
//         add_contents(&mut content_ui);
//     });
// }
