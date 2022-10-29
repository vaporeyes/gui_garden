use eframe::egui;
use egui::{Color32, RichText, Ui};

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
    calc_value: String,
    label_calc_cur_value: String,
    calc_plus_clicked: bool,
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            // Example stuff:
            label: "Hello World!".to_owned(),
            value: 2.7,
            about_is_open: true,
            calc_is_open: true,
            calc_value: "".to_string(),
            label_calc_cur_value: "".to_string(),
            calc_plus_clicked: false,
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

const ORANGE: Color32 = Color32::from_rgb(244, 166, 52);
const GRAY: Color32 = Color32::from_rgb(95, 95, 104);
const DKGRAY: Color32 = Color32::from_rgb(63, 63, 70);
const BLACK: Color32 = Color32::from_rgb(0, 0, 0);
const WHITE: Color32 = Color32::from_rgb(255, 255, 255);

impl eframe::App for TemplateApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.calc_plus_clicked = false;
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let Self {
            label: _,
            value: _,
            calc_is_open: _,
            about_is_open: _,
            calc_value: _,
            label_calc_cur_value: _,
            calc_plus_clicked: _,
        } = self;

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

        egui::Window::new("About Me")
            .open(&mut self.about_is_open)
            .show(ctx, |ui| {
                egui::TopBottomPanel::top("top_panel")
                    .resizable(true)
                    .min_height(32.0)
                    .show_inside(ui, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            ui.with_layout(
                                egui::Layout::top_down(egui::Align::LEFT).with_cross_justify(true),
                                |ui| {
                                    if ui.label(
                                        egui::RichText::new("My name is Josh and I do DevOPs for a living. These are some Rust egui tests.").weak(),
                                    ).double_clicked() {
                                        //
                                    }
                                },
                            );
                        });
                    });
                ui.label("Life Skills");
                egui::Grid::new("life_skills")
                    .num_columns(2)
                    .spacing([40.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label("Drawing");
                        let progress = 300.0 / 360.0;
                        let progress_bar = egui::ProgressBar::new(progress).show_percentage();
                        ui.add(progress_bar);
                        ui.end_row();
                        ui.label("Painting");
                        let progress = 250.0 / 360.0;
                        let progress_bar = egui::ProgressBar::new(progress).show_percentage();
                        ui.add(progress_bar);
                        ui.end_row();
                        ui.label("Cooking");
                        let progress = 290.0 / 360.0;
                        let progress_bar = egui::ProgressBar::new(progress).show_percentage();
                        ui.add(progress_bar);
                        ui.end_row();
                        ui.label("Model Building: Plastic Models");
                        let progress = 200.0 / 360.0;
                        let progress_bar = egui::ProgressBar::new(progress).show_percentage();
                        ui.add(progress_bar);
                        ui.end_row();
                    });
                ui.separator();
                ui.label("Work Skills");
                egui::Grid::new("work_skills")
                    .num_columns(2)
                    .spacing([150.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label("Python");
                        let progress = 234.0 / 360.0;
                        let progress_bar = egui::ProgressBar::new(progress).show_percentage();
                        ui.add(progress_bar);
                        ui.end_row();
                        ui.label("Javascript");
                        let progress = 126.0 / 360.0;
                        let progress_bar = egui::ProgressBar::new(progress).show_percentage();
                        ui.add(progress_bar);
                        ui.end_row();
                        ui.label("Rust");
                        let progress = 60.0 / 360.0;
                        let progress_bar = egui::ProgressBar::new(progress).show_percentage();
                        ui.add(progress_bar);
                        ui.end_row();
                        ui.label("Elixir");
                        let progress = 85.0 / 360.0;
                        let progress_bar = egui::ProgressBar::new(progress).show_percentage();
                        ui.add(progress_bar);
                        ui.end_row();
                    });
            });
        // calculator(ctx, &mut self.calc_is_open);
        egui::Window::new("A Calculator")
            .open(&mut self.calc_is_open)
            .fixed_size([433.0, 433.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    egui::TextEdit::singleline(&mut self.calc_value)
                        .desired_width(f32::INFINITY)
                        .show(ui)
                });
                ui.horizontal(|ui| ui.label(self.label_calc_cur_value.to_string()));
                egui::Grid::new("calc_buttons_row_1")
                    .num_columns(4)
                    .spacing([40.0, 4.0])
                    .show(ui, |ui| {
                        if ui
                            .button(
                                RichText::new("   C   ")
                                    .size(20.0)
                                    .monospace()
                                    .color(WHITE)
                                    .background_color(DKGRAY),
                            )
                            .clicked()
                        {
                            self.calc_value = 0.to_string();
                            self.label_calc_cur_value = "".to_string();
                            self.calc_plus_clicked = false;
                        };
                        if ui
                            .button(
                                RichText::new("   ±   ")
                                    .size(20.0)
                                    .monospace()
                                    .color(WHITE)
                                    .background_color(DKGRAY),
                            )
                            .clicked()
                        {
                            //
                        }
                        if ui
                            .button(
                                RichText::new("   %   ")
                                    .size(20.0)
                                    .monospace()
                                    .color(WHITE)
                                    .background_color(DKGRAY),
                            )
                            .clicked()
                        {
                            //
                        }
                        if ui
                            .button(
                                RichText::new("   ÷   ")
                                    .size(20.0)
                                    .monospace()
                                    .color(WHITE)
                                    .background_color(ORANGE),
                            )
                            .clicked()
                        {
                            self.label_calc_cur_value = self.calc_value.to_string();
                        }
                    });
                egui::Grid::new("calc_buttons_row_2")
                    .num_columns(4)
                    .spacing([40.0, 4.0])
                    .show(ui, |ui| {
                        if ui
                            .button(
                                RichText::new("   7   ")
                                    .size(20.0)
                                    .monospace()
                                    .color(WHITE)
                                    .background_color(GRAY),
                            )
                            .clicked()
                        {
                            if self.calc_plus_clicked {
                                self.calc_value = format!("{}", "7");
                                self.calc_plus_clicked = false;
                            } else {
                                if self.calc_value == "0" {
                                    self.calc_value = format!("{}", "7")
                                } else {
                                    self.calc_value = format!("{}{}", &self.calc_value, "7")
                                }
                            }
                        };
                        if ui
                            .button(
                                RichText::new("   8   ")
                                    .size(20.0)
                                    .monospace()
                                    .color(WHITE)
                                    .background_color(GRAY),
                            )
                            .clicked()
                        {
                            if self.calc_plus_clicked {
                                self.calc_value = format!("{}", "8");
                                self.calc_plus_clicked = false;
                            } else {
                                if self.calc_value == "0" {
                                    self.calc_value = format!("{}", "8")
                                } else {
                                    self.calc_value = format!("{}{}", &self.calc_value, "8")
                                }
                            }
                        }
                        if ui
                            .button(
                                RichText::new("   9   ")
                                    .size(20.0)
                                    .monospace()
                                    .color(WHITE)
                                    .background_color(GRAY),
                            )
                            .clicked()
                        {
                            if self.calc_plus_clicked {
                                self.calc_value = format!("{}", "9");
                                self.calc_plus_clicked = false;
                            } else {
                                if self.calc_value == "0" {
                                    self.calc_value = format!("{}", "9")
                                } else {
                                    self.calc_value = format!("{}{}", &self.calc_value, "9")
                                }
                            }
                        }
                        ui.button(
                            RichText::new("   x   ")
                                .size(20.0)
                                .monospace()
                                .color(WHITE)
                                .background_color(ORANGE),
                        )
                        .clicked();
                    });
                egui::Grid::new("calc_buttons_row_3")
                    .num_columns(4)
                    .spacing([40.0, 4.0])
                    .show(ui, |ui| {
                        if ui
                            .button(
                                RichText::new("   4   ")
                                    .size(20.0)
                                    .monospace()
                                    .color(WHITE)
                                    .background_color(GRAY),
                            )
                            .clicked()
                        {
                            if self.calc_plus_clicked {
                                self.calc_value = format!("{}", "4");
                                self.calc_plus_clicked = false;
                            } else {
                                if self.calc_value == "0" {
                                    self.calc_value = format!("{}", "4")
                                } else {
                                    self.calc_value = format!("{}{}", &self.calc_value, "4")
                                }
                            }
                        };
                        if ui
                            .button(
                                RichText::new("   5   ")
                                    .size(20.0)
                                    .monospace()
                                    .color(WHITE)
                                    .background_color(GRAY),
                            )
                            .clicked()
                        {
                            if self.calc_plus_clicked {
                                self.calc_value = format!("{}", "5");
                                self.calc_plus_clicked = false;
                            } else {
                                if self.calc_value == "0" {
                                    self.calc_value = format!("{}", "5")
                                } else {
                                    self.calc_value = format!("{}{}", &self.calc_value, "5")
                                }
                            }
                        };
                        if ui
                            .button(
                                RichText::new("   6   ")
                                    .size(20.0)
                                    .monospace()
                                    .color(WHITE)
                                    .background_color(GRAY),
                            )
                            .clicked()
                        {
                            if self.calc_plus_clicked {
                                self.calc_value = format!("{}", "6");
                                self.calc_plus_clicked = false;
                            } else {
                                if self.calc_value == "0" {
                                    self.calc_value = format!("{}", "6")
                                } else {
                                    self.calc_value = format!("{}{}", &self.calc_value, "6")
                                }
                            }
                        };
                        ui.button(
                            RichText::new("   -   ")
                                .size(20.0)
                                .monospace()
                                .color(WHITE)
                                .background_color(ORANGE),
                        )
                        .clicked();
                    });
                egui::Grid::new("calc_buttons_row_4")
                    .num_columns(4)
                    .spacing([40.0, 4.0])
                    .show(ui, |ui| {
                        if ui
                            .button(
                                RichText::new("   1   ")
                                    .size(20.0)
                                    .monospace()
                                    .color(WHITE)
                                    .background_color(GRAY),
                            )
                            .clicked()
                        {
                            if self.calc_plus_clicked {
                                self.calc_value = format!("{}", "1");
                                self.calc_plus_clicked = false;
                            } else {
                                if self.calc_value == "0" {
                                    self.calc_value = format!("{}", "1")
                                } else {
                                    self.calc_value = format!("{}{}", &self.calc_value, "1")
                                }
                            }
                        };
                        if ui
                            .button(
                                RichText::new("   2   ")
                                    .size(20.0)
                                    .monospace()
                                    .color(WHITE)
                                    .background_color(GRAY),
                            )
                            .clicked()
                        {
                            if self.calc_plus_clicked {
                                self.calc_value = format!("{}", "2");
                                self.calc_plus_clicked = false;
                            } else {
                                if self.calc_value == "0" {
                                    self.calc_value = format!("{}", "2")
                                } else {
                                    self.calc_value = format!("{}{}", &self.calc_value, "2")
                                }
                            }
                        };
                        if ui
                            .button(
                                RichText::new("   3   ")
                                    .size(20.0)
                                    .monospace()
                                    .color(WHITE)
                                    .background_color(GRAY),
                            )
                            .clicked()
                        {
                            if self.calc_plus_clicked {
                                self.calc_value = format!("{}", "3");
                                self.calc_plus_clicked = false;
                            } else {
                                if self.calc_value == "0" {
                                    self.calc_value = format!("{}", "3")
                                } else {
                                    self.calc_value = format!("{}{}", &self.calc_value, "3")
                                }
                            }
                        };
                        if ui
                            .button(
                                RichText::new("   +   ")
                                    .size(20.0)
                                    .monospace()
                                    .color(WHITE)
                                    .background_color(ORANGE),
                            )
                            .clicked()
                        {
                            self.calc_plus_clicked = true;
                            if self.calc_value.contains(".") {
                                // parse as float
                            } else {
                                // parse as int
                                if self.label_calc_cur_value.is_empty() {
                                    self.label_calc_cur_value = self.calc_value.to_string();
                                } else {
                                    let tmp_calc_value: i128 =
                                        self.calc_value.parse::<i128>().unwrap();
                                    let tmp_label_calc_value: i128 =
                                        self.label_calc_cur_value.parse::<i128>().unwrap();
                                    let new_label_value = tmp_calc_value + tmp_label_calc_value;
                                    self.label_calc_cur_value = new_label_value.to_string();
                                }
                            }
                        }
                    });
                // 244, 166, 52
                // egui::Color32::
                egui::Grid::new("calc_buttons_row_5")
                    .num_columns(3)
                    .spacing([40.0, 4.0])
                    .show(ui, |ui| {
                        if ui
                            .button(
                                RichText::new("          0          ")
                                    .size(18.0)
                                    .monospace()
                                    .color(WHITE)
                                    .background_color(GRAY),
                            )
                            .clicked()
                        {
                            if self.calc_value == "0" {
                                self.calc_value = format!("{}", "0")
                            } else {
                                self.calc_value = format!("{}{}", &self.calc_value, "0")
                            }
                        }
                        if ui
                            .button(
                                RichText::new("   .   ")
                                    .size(20.0)
                                    .monospace()
                                    .color(WHITE)
                                    .background_color(GRAY),
                            )
                            .clicked()
                        {
                            if self.calc_value.contains(".") {
                            } else {
                                self.calc_value = format!("{}{}", &self.calc_value, ".")
                            }
                        }
                        if ui
                            .button(
                                RichText::new("   =   ")
                                    .size(20.0)
                                    .monospace()
                                    .color(WHITE)
                                    .background_color(ORANGE),
                            )
                            .clicked()
                        {}
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
