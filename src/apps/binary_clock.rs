// Binary clock — LED-matrix style, tracking the current time either as
// BCD (binary-coded decimal, one column per digit of HH:MM:SS) or as pure
// 6-bit binary per field. Lit LEDs pick up today's Poline accent so the
// clock retints with dawn / midday / dusk.
//
// Shares the same contract as `FractalClock`: a `ui(&mut self, ui,
// seconds_since_midnight)` method that paints into the containing ui's
// layer and requests repaints while not paused.

use egui::{
    containers::*, widgets::*, Align2, Color32, FontId, InputState, Painter, Pos2, Rect, Shape,
    Stroke, Ui, Vec2,
};

use crate::palette;

#[derive(PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct BinaryClock {
    paused: bool,
    time: f64,
    show_labels: bool,
    show_decimal_readout: bool,
    pure_binary: bool,
    led_size: f32,
}

impl Default for BinaryClock {
    fn default() -> Self {
        Self {
            paused: false,
            time: 0.0,
            show_labels: true,
            show_decimal_readout: true,
            pure_binary: false,
            led_size: 20.0,
        }
    }
}

impl BinaryClock {
    pub fn ui(&mut self, ui: &mut Ui, seconds_since_midnight: Option<f64>) {
        if !self.paused {
            self.time = seconds_since_midnight
                .unwrap_or_else(|| ui.input(|i: &InputState| i.time));
            ui.ctx().request_repaint();
        }

        let painter = Painter::new(
            ui.ctx().clone(),
            ui.layer_id(),
            ui.available_rect_before_wrap(),
        );
        self.paint(&painter);
        // Claim the full area so the settings popup floats above, not below.
        ui.expand_to_include_rect(painter.clip_rect());

        Frame::popup(ui.style())
            .stroke(Stroke::NONE)
            .show(ui, |ui| {
                ui.set_max_width(260.0);
                CollapsingHeader::new("Binary Clock Settings")
                    .show(ui, |ui| self.options_ui(ui, seconds_since_midnight));
            });
    }

    fn options_ui(&mut self, ui: &mut Ui, seconds_since_midnight: Option<f64>) {
        if seconds_since_midnight.is_some() {
            let (h, m, s) = self.hms();
            ui.label(format!("Local time: {:02}:{:02}:{:02}", h, m, s));
        } else {
            ui.label("(running on input.time, not wall clock)");
        }
        ui.checkbox(&mut self.paused, "Paused");
        ui.checkbox(&mut self.show_labels, "Show labels");
        ui.checkbox(&mut self.show_decimal_readout, "Decimal readout");
        ui.checkbox(&mut self.pure_binary, "Pure binary (6-bit)");
        ui.add(Slider::new(&mut self.led_size, 10.0..=40.0).text("LED size"));
        egui::reset_button(ui, self, "Reset");
        ui.label(
            egui::RichText::new(
                "Lit-LED color follows today's Poline accent.",
            )
            .small()
            .weak(),
        );
    }

    fn hms(&self) -> (u32, u32, u32) {
        let total = self.time.rem_euclid(24.0 * 3600.0);
        let h = (total / 3600.0).floor() as u32;
        let m = ((total % 3600.0) / 60.0).floor() as u32;
        let s = (total % 60.0).floor() as u32;
        (h, m, s)
    }

    fn paint(&self, painter: &Painter) {
        let clip = painter.clip_rect();
        let accent = palette::accent_now();
        let unlit = Color32::from_rgb(50, 50, 50);
        let glow = accent.linear_multiply(0.28);

        let (h, m, s) = self.hms();
        if self.pure_binary {
            self.paint_pure(painter, clip, h, m, s, accent, unlit, glow);
        } else {
            self.paint_bcd(painter, clip, h, m, s, accent, unlit, glow);
        }

        if self.show_decimal_readout {
            let text = format!("{:02}:{:02}:{:02}", h, m, s);
            let font = FontId::monospace((self.led_size * 0.9).max(12.0));
            painter.text(
                clip.center_bottom() - Vec2::new(0.0, self.led_size * 0.6),
                Align2::CENTER_BOTTOM,
                text,
                font,
                Color32::from_gray(170),
            );
        }
    }

    // ---------- BCD layout ----------

    fn paint_bcd(
        &self,
        painter: &Painter,
        clip: Rect,
        h: u32,
        m: u32,
        s: u32,
        accent: Color32,
        unlit: Color32,
        glow: Color32,
    ) {
        // Columns: (digit_value, bits_used, short_label)
        let columns: [(u32, u32, &str); 6] = [
            (h / 10, 2, "h"),
            (h % 10, 4, "h"),
            (m / 10, 3, "m"),
            (m % 10, 4, "m"),
            (s / 10, 3, "s"),
            (s % 10, 4, "s"),
        ];
        // Extra gap between h/m and m/s column pairs for readability.
        let pair_gaps: [f32; 6] = [0.0, 0.0, 14.0, 0.0, 14.0, 0.0];

        let gap = 6.0;
        let cell = self.led_size + gap;
        let grid_w = 6.0 * cell + pair_gaps.iter().sum::<f32>();
        let grid_h = 4.0 * cell; // 4 rows of BCD (max bit position = 3)
        let label_space_bottom = if self.show_labels { 22.0 } else { 0.0 };
        let label_space_left = if self.show_labels { 18.0 } else { 0.0 };

        let total = Vec2::new(grid_w + label_space_left, grid_h + label_space_bottom);
        let origin = Pos2::new(
            clip.center().x - total.x / 2.0,
            clip.center().y - total.y / 2.0,
        );
        let grid_origin = origin + Vec2::new(label_space_left, 0.0);

        // Row bit labels on the left (8, 4, 2, 1).
        if self.show_labels {
            let bit_values = [8, 4, 2, 1];
            for (row, val) in bit_values.iter().enumerate() {
                let y = grid_origin.y + row as f32 * cell + self.led_size / 2.0 + gap / 2.0;
                painter.text(
                    Pos2::new(grid_origin.x - 4.0, y),
                    Align2::RIGHT_CENTER,
                    val.to_string(),
                    FontId::monospace(11.0),
                    Color32::from_gray(120),
                );
            }
        }

        // LEDs
        let mut col_x_offset = 0.0;
        for (col, (value, n_bits, _)) in columns.iter().enumerate() {
            col_x_offset += pair_gaps[col];
            let col_x = grid_origin.x + col as f32 * cell + col_x_offset;
            let cx = col_x + self.led_size / 2.0 + gap / 2.0;
            for bit in 0..*n_bits {
                let row = 3 - bit; // bit 0 (value 1) is bottom row
                let cy = grid_origin.y + row as f32 * cell + self.led_size / 2.0 + gap / 2.0;
                let lit = (value >> bit) & 1 == 1;
                self.draw_led(painter, Pos2::new(cx, cy), lit, accent, unlit, glow);
            }
        }

        // Column labels along the bottom: h h  m m  s s
        if self.show_labels {
            let mut col_x_offset2 = 0.0;
            for (col, (_, _, label)) in columns.iter().enumerate() {
                col_x_offset2 += pair_gaps[col];
                let col_x = grid_origin.x + col as f32 * cell + col_x_offset2;
                let cx = col_x + self.led_size / 2.0 + gap / 2.0;
                painter.text(
                    Pos2::new(cx, grid_origin.y + grid_h + 4.0),
                    Align2::CENTER_TOP,
                    *label,
                    FontId::monospace(12.0),
                    Color32::from_gray(130),
                );
            }
        }
    }

    // ---------- pure binary layout ----------

    fn paint_pure(
        &self,
        painter: &Painter,
        clip: Rect,
        h: u32,
        m: u32,
        s: u32,
        accent: Color32,
        unlit: Color32,
        glow: Color32,
    ) {
        // Three rows (hours, minutes, seconds) × 6 bits each, MSB left.
        let rows: [(u32, &str); 3] = [(h, "h"), (m, "m"), (s, "s")];
        let bits: u32 = 6;

        let gap = 6.0;
        let cell = self.led_size + gap;
        let grid_w = bits as f32 * cell;
        let grid_h = 3.0 * cell;
        let label_space_bottom = 0.0;
        let label_space_left = if self.show_labels { 20.0 } else { 0.0 };
        let total = Vec2::new(grid_w + label_space_left, grid_h + label_space_bottom);
        let origin = Pos2::new(
            clip.center().x - total.x / 2.0,
            clip.center().y - total.y / 2.0,
        );
        let grid_origin = origin + Vec2::new(label_space_left, 0.0);

        for (row_idx, (value, label)) in rows.iter().enumerate() {
            let cy = grid_origin.y + row_idx as f32 * cell + self.led_size / 2.0 + gap / 2.0;
            if self.show_labels {
                painter.text(
                    Pos2::new(grid_origin.x - 6.0, cy),
                    Align2::RIGHT_CENTER,
                    *label,
                    FontId::monospace(12.0),
                    Color32::from_gray(130),
                );
            }
            for bit in 0..bits {
                // Left-most LED is the high bit (value 32).
                let col = (bits - 1 - bit) as f32;
                let cx = grid_origin.x + col * cell + self.led_size / 2.0 + gap / 2.0;
                let lit = (value >> bit) & 1 == 1;
                self.draw_led(painter, Pos2::new(cx, cy), lit, accent, unlit, glow);
            }
        }
    }

    fn draw_led(
        &self,
        painter: &Painter,
        pos: Pos2,
        lit: bool,
        accent: Color32,
        unlit: Color32,
        glow: Color32,
    ) {
        let r = self.led_size / 2.0;
        if lit {
            // Two-pass glow + core so the lit state feels warm rather
            // than a solid disc.
            painter.add(Shape::circle_filled(pos, r + 4.0, glow));
            painter.circle_filled(pos, r, accent);
        } else {
            painter.circle_stroke(pos, r, Stroke::new(1.0, unlit));
        }
    }
}
