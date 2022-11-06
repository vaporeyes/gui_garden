use egui::{Color32, RichText, Ui};

const ORANGE: Color32 = Color32::from_rgb(244, 166, 52);
const GRAY: Color32 = Color32::from_rgb(95, 95, 104);
const DKGRAY: Color32 = Color32::from_rgb(63, 63, 70);
// const BLACK: Color32 = Color32::from_rgb(0, 0, 0);
// const DKDKGRAY: Color32 = Color32::from_rgb(27, 27, 27);
const WHITE: Color32 = Color32::from_rgb(255, 255, 255);
// const DKOLIVE: Color32 = Color32::from_rgb(64, 61, 33);

#[derive(PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct Calculator {
    calc_value: String,
    label_calc_cur_value: String,
    operator_button_clicked: bool,
    calc_is_open: bool,
}

impl Default for Calculator {
    fn default() -> Self {
        Self {
            calc_value: "0".to_string(),
            label_calc_cur_value: "".to_string(),
            operator_button_clicked: false,
            calc_is_open: false,
        }
    }
}

impl Calculator {
    pub fn ui(&mut self, ui: &mut Ui) {
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
                    self.operator_button_clicked = false;
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
                        RichText::new("   /   ")
                            .size(20.0)
                            .monospace()
                            .color(WHITE)
                            .background_color(ORANGE),
                    )
                    .clicked()
                {
                    self.operator_button_clicked = true;
                    if self.label_calc_cur_value == "0" {
                        self.label_calc_cur_value = "Not a number!".to_string();
                    } else if self.label_calc_cur_value == "".to_string() {
                        self.label_calc_cur_value = self.calc_value.to_string()
                    } else {
                        self.label_calc_cur_value =
                            process_calculation(&self.calc_value, &self.label_calc_cur_value, "/");
                    }
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
                    if self.operator_button_clicked {
                        self.calc_value = format!("{}", "7");
                        self.operator_button_clicked = false;
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
                    if self.operator_button_clicked {
                        self.calc_value = format!("{}", "8");
                        self.operator_button_clicked = false;
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
                    if self.operator_button_clicked {
                        self.calc_value = format!("{}", "9");
                        self.operator_button_clicked = false;
                    } else {
                        if self.calc_value == "0" {
                            self.calc_value = format!("{}", "9")
                        } else {
                            self.calc_value = format!("{}{}", &self.calc_value, "9")
                        }
                    }
                }
                if ui
                    .button(
                        RichText::new("   x   ")
                            .size(20.0)
                            .monospace()
                            .color(WHITE)
                            .background_color(ORANGE),
                    )
                    .clicked()
                {
                    self.operator_button_clicked = true;
                    self.label_calc_cur_value =
                        process_calculation(&self.calc_value, &self.label_calc_cur_value, "*");
                }
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
                    if self.operator_button_clicked {
                        self.calc_value = format!("{}", "4");
                        self.operator_button_clicked = false;
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
                    if self.operator_button_clicked {
                        self.calc_value = format!("{}", "5");
                        self.operator_button_clicked = false;
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
                    if self.operator_button_clicked {
                        self.calc_value = format!("{}", "6");
                        self.operator_button_clicked = false;
                    } else {
                        if self.calc_value == "0" {
                            self.calc_value = format!("{}", "6")
                        } else {
                            self.calc_value = format!("{}{}", &self.calc_value, "6")
                        }
                    }
                };
                if ui
                    .button(
                        RichText::new("   -   ")
                            .size(20.0)
                            .monospace()
                            .color(WHITE)
                            .background_color(ORANGE),
                    )
                    .clicked()
                {
                    self.operator_button_clicked = true;
                    self.label_calc_cur_value =
                        process_calculation(&self.calc_value, &self.label_calc_cur_value, "-");
                }
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
                    if self.operator_button_clicked {
                        self.calc_value = format!("{}", "1");
                        self.operator_button_clicked = false;
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
                    if self.operator_button_clicked {
                        self.calc_value = format!("{}", "2");
                        self.operator_button_clicked = false;
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
                    if self.operator_button_clicked {
                        self.calc_value = format!("{}", "3");
                        self.operator_button_clicked = false;
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
                    self.operator_button_clicked = true;
                    self.label_calc_cur_value =
                        process_calculation(&self.calc_value, &self.label_calc_cur_value, "+");
                }
            });
        egui::Grid::new("calc_buttons_row_5")
            .num_columns(3)
            .spacing([38.0, 4.0])
            .show(ui, |ui| {
                if ui
                    .button(
                        RichText::new("         0          ")
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
    }
}

fn process_calculation(calc_value_1: &String, calc_value_2: &String, operator: &str) -> String {
    if calc_value_1.contains(".") {
        // parse as float
        "".to_string()
    } else {
        let mut tmp_value: i128 = 0;
        let tmp_calc_value_1: i128 = calc_value_1.parse::<i128>().unwrap_or(0);
        let tmp_calc_value_2: i128 = calc_value_2.parse::<i128>().unwrap_or(0);
        if operator == "+" {
            tmp_value = tmp_calc_value_1 + tmp_calc_value_2
        }
        if operator == "-" {
            tmp_value = tmp_calc_value_1 - tmp_calc_value_2
        }
        if operator == "/" {
            tmp_value =  tmp_calc_value_2 / tmp_calc_value_1
        }
        if operator == "*" {
            if multiply_will_overflow(tmp_calc_value_1, tmp_calc_value_2) {
                return "product too large!".to_string();
            } else {
                tmp_value = tmp_calc_value_1 * tmp_calc_value_2
            }
        }
        tmp_value.to_string()
    }
}

// BIG NUMBAHHHHHHHHHS
fn multiply_will_overflow(x: i128, y: i128) -> bool {
    x.checked_mul(y).is_none()
}
