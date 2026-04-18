use egui::{Color32, RichText, Ui};

use crate::palette;

// Fallback accent when we don't want to burn a clock lookup on every paint.
// The live operator color comes from `palette::accent_now()` each frame,
// which lets dawn/dusk subtly retint the operator keys without touching
// the iOS-familiar neutrals on digits and actions.
const GRAY: Color32 = Color32::from_rgb(95, 95, 104);
const DKGRAY: Color32 = Color32::from_rgb(63, 63, 70);
const WHITE: Color32 = Color32::from_rgb(255, 255, 255);
const DISPLAY_BG: Color32 = Color32::from_rgb(18, 18, 18);

const BTN: f32 = 60.0;

/// Arithmetic operators the calculator understands.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

impl Op {
    fn symbol(self) -> &'static str {
        match self {
            Op::Add => "+",
            Op::Sub => "−",
            Op::Mul => "×",
            Op::Div => "÷",
        }
    }
}

/// Where we are in the "left op right = result" flow. Tracked explicitly so
/// digit / operator / equals presses can make consistent decisions about
/// whether to extend the current display or start a new number.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Entry {
    /// Typing the first (or only) operand. Digits append to `display`.
    LeftOperand,
    /// An operator was just pressed. The next digit replaces `display`.
    RightOperand,
    /// `=` was just pressed. The next digit starts a fresh calculation
    /// (accumulator + pending op both cleared).
    Evaluated,
}

pub struct Calculator {
    /// The number currently on screen.
    display: String,
    /// Left operand carried over from the most recent operator press.
    accumulator: Option<f64>,
    /// Operator waiting for its right operand.
    pending_op: Option<Op>,
    /// Entry state (see `Entry`).
    entry: Entry,
    /// Latched on divide-by-zero / overflow / NaN. Cleared by `C`.
    error: bool,
}

impl Default for Calculator {
    fn default() -> Self {
        Self {
            display: "0".to_string(),
            accumulator: None,
            pending_op: None,
            entry: Entry::LeftOperand,
            error: false,
        }
    }
}

impl Calculator {
    pub fn ui(&mut self, ui: &mut Ui) {
        self.handle_keyboard(ui);
        self.draw_display(ui);
        ui.add_space(6.0);
        self.draw_keypad(ui);
    }

    // ---------- rendering ----------

    fn draw_display(&self, ui: &mut Ui) {
        // Secondary line: "5 +" style pending-expression preview.
        let preview = match (self.accumulator, self.pending_op) {
            (Some(acc), Some(op)) => format!("{} {}", format_number(acc), op.symbol()),
            _ => String::new(),
        };

        let frame = egui::Frame::NONE
            .fill(DISPLAY_BG)
            .inner_margin(egui::Margin::symmetric(14, 10))
            .corner_radius(egui::CornerRadius::same(6))
            .stroke(egui::Stroke::new(1.0, palette::accent_now()));
        frame.show(ui, |ui| {
            ui.with_layout(
                egui::Layout::top_down(egui::Align::RIGHT).with_cross_justify(true),
                |ui| {
                    ui.label(
                        RichText::new(if preview.is_empty() { " " } else { &preview })
                            .monospace()
                            .size(14.0)
                            .color(Color32::from_gray(140)),
                    );
                    ui.label(
                        RichText::new(&self.display)
                            .monospace()
                            .size(32.0)
                            .color(WHITE),
                    );
                },
            );
        });
    }

    fn draw_keypad(&mut self, ui: &mut Ui) {
        // Row 1: C  ±  %  ÷
        ui.horizontal(|ui| {
            if action_button(ui, "C") {
                self.press_clear();
            }
            if action_button(ui, "±") {
                self.press_negate();
            }
            if action_button(ui, "%") {
                self.press_percent();
            }
            if operator_button(ui, "÷") {
                self.press_op(Op::Div);
            }
        });
        // Row 2: 7 8 9 ×
        ui.horizontal(|ui| {
            if digit_button(ui, "7") {
                self.press_digit('7');
            }
            if digit_button(ui, "8") {
                self.press_digit('8');
            }
            if digit_button(ui, "9") {
                self.press_digit('9');
            }
            if operator_button(ui, "×") {
                self.press_op(Op::Mul);
            }
        });
        // Row 3: 4 5 6 −
        ui.horizontal(|ui| {
            if digit_button(ui, "4") {
                self.press_digit('4');
            }
            if digit_button(ui, "5") {
                self.press_digit('5');
            }
            if digit_button(ui, "6") {
                self.press_digit('6');
            }
            if operator_button(ui, "−") {
                self.press_op(Op::Sub);
            }
        });
        // Row 4: 1 2 3 +
        ui.horizontal(|ui| {
            if digit_button(ui, "1") {
                self.press_digit('1');
            }
            if digit_button(ui, "2") {
                self.press_digit('2');
            }
            if digit_button(ui, "3") {
                self.press_digit('3');
            }
            if operator_button(ui, "+") {
                self.press_op(Op::Add);
            }
        });
        // Row 5: 0 (wide)  .  =
        ui.horizontal(|ui| {
            let spacing = ui.spacing().item_spacing.x;
            let wide = BTN * 2.0 + spacing;
            if wide_digit_button(ui, "0", wide) {
                self.press_digit('0');
            }
            if digit_button(ui, ".") {
                self.press_decimal();
            }
            if operator_button(ui, "=") {
                self.press_equals();
            }
        });
    }

    // ---------- input handling ----------

    fn handle_keyboard(&mut self, ui: &mut Ui) {
        // Only react to keys when no other widget has focus — otherwise
        // typing in, say, the notes search would also drive the calculator.
        if ui.ctx().memory(|m| m.focused().is_some()) {
            return;
        }

        let events = ui.input(|i| i.events.clone());
        for event in events {
            match event {
                egui::Event::Text(t) => {
                    for ch in t.chars() {
                        match ch {
                            '0'..='9' => self.press_digit(ch),
                            '.' => self.press_decimal(),
                            '+' => self.press_op(Op::Add),
                            '-' => self.press_op(Op::Sub),
                            '*' | 'x' | 'X' => self.press_op(Op::Mul),
                            '/' => self.press_op(Op::Div),
                            '%' => self.press_percent(),
                            '=' => self.press_equals(),
                            'n' | 'N' => self.press_negate(),
                            _ => {}
                        }
                    }
                }
                egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: _,
                    ..
                } => match key {
                    egui::Key::Enter => self.press_equals(),
                    egui::Key::Escape => self.press_clear(),
                    egui::Key::Backspace => self.press_backspace(),
                    _ => {}
                },
                _ => {}
            }
        }
    }

    // ---------- state transitions ----------

    fn press_digit(&mut self, d: char) {
        if self.error {
            return;
        }
        match self.entry {
            Entry::LeftOperand => {
                if self.display == "0" {
                    self.display = d.to_string();
                } else if self.display == "-0" {
                    self.display = format!("-{}", d);
                } else {
                    self.display.push(d);
                }
            }
            Entry::RightOperand => {
                self.display = d.to_string();
                self.entry = Entry::LeftOperand;
            }
            Entry::Evaluated => {
                self.display = d.to_string();
                self.accumulator = None;
                self.pending_op = None;
                self.entry = Entry::LeftOperand;
            }
        }
    }

    fn press_decimal(&mut self) {
        if self.error {
            return;
        }
        match self.entry {
            Entry::LeftOperand => {
                if !self.display.contains('.') {
                    self.display.push('.');
                }
            }
            Entry::RightOperand => {
                self.display = "0.".to_string();
                self.entry = Entry::LeftOperand;
            }
            Entry::Evaluated => {
                self.display = "0.".to_string();
                self.accumulator = None;
                self.pending_op = None;
                self.entry = Entry::LeftOperand;
            }
        }
    }

    fn press_op(&mut self, op: Op) {
        if self.error {
            return;
        }
        match self.entry {
            Entry::LeftOperand => {
                // If a previous operator is pending, chain it first so
                // `5 + 3 + 2` becomes 8 + 2 = 10.
                if let (Some(acc), Some(pending)) = (self.accumulator, self.pending_op) {
                    let rhs = self.display.parse::<f64>().unwrap_or(0.0);
                    match apply_op(acc, rhs, pending) {
                        Ok(result) => {
                            self.accumulator = Some(result);
                            self.display = format_number(result);
                        }
                        Err(()) => {
                            self.set_error();
                            return;
                        }
                    }
                } else {
                    self.accumulator = Some(self.display.parse::<f64>().unwrap_or(0.0));
                }
            }
            Entry::RightOperand => {
                // Double operator press — just swap which op is pending.
            }
            Entry::Evaluated => {
                self.accumulator = Some(self.display.parse::<f64>().unwrap_or(0.0));
            }
        }
        self.pending_op = Some(op);
        self.entry = Entry::RightOperand;
    }

    fn press_equals(&mut self) {
        if self.error {
            return;
        }
        if let (Some(acc), Some(pending)) = (self.accumulator, self.pending_op) {
            let rhs = self.display.parse::<f64>().unwrap_or(0.0);
            match apply_op(acc, rhs, pending) {
                Ok(result) => {
                    self.display = format_number(result);
                    self.accumulator = None;
                    self.pending_op = None;
                    self.entry = Entry::Evaluated;
                }
                Err(()) => self.set_error(),
            }
        }
    }

    fn press_negate(&mut self) {
        if self.error {
            return;
        }
        if self.display == "0" || self.display == "0." {
            return;
        }
        if let Some(stripped) = self.display.strip_prefix('-') {
            self.display = stripped.to_string();
        } else {
            self.display = format!("-{}", self.display);
        }
    }

    fn press_percent(&mut self) {
        if self.error {
            return;
        }
        let n = self.display.parse::<f64>().unwrap_or(0.0);
        self.display = format_number(n / 100.0);
    }

    fn press_clear(&mut self) {
        self.display = "0".to_string();
        self.accumulator = None;
        self.pending_op = None;
        self.entry = Entry::LeftOperand;
        self.error = false;
    }

    fn press_backspace(&mut self) {
        if self.error {
            self.press_clear();
            return;
        }
        if self.entry != Entry::LeftOperand {
            return;
        }
        self.display.pop();
        if self.display.is_empty() || self.display == "-" {
            self.display = "0".to_string();
        }
    }

    fn set_error(&mut self) {
        self.display = "Error".to_string();
        self.accumulator = None;
        self.pending_op = None;
        self.entry = Entry::LeftOperand;
        self.error = true;
    }
}

// ---------- helpers ----------

fn apply_op(a: f64, b: f64, op: Op) -> Result<f64, ()> {
    let r = match op {
        Op::Add => a + b,
        Op::Sub => a - b,
        Op::Mul => a * b,
        Op::Div => {
            if b == 0.0 {
                return Err(());
            }
            a / b
        }
    };
    if !r.is_finite() {
        return Err(());
    }
    Ok(r)
}

/// Format a number for display. Whole numbers that fit safely in i64 are
/// shown without a decimal point; everything else trims trailing zeros.
fn format_number(n: f64) -> String {
    if !n.is_finite() {
        return "Error".to_string();
    }
    if n == 0.0 {
        return "0".to_string();
    }
    if n.fract() == 0.0 && n.abs() < 1e15 {
        return format!("{}", n as i64);
    }
    let s = format!("{:.10}", n);
    let trimmed = s.trim_end_matches('0').trim_end_matches('.');
    trimmed.to_string()
}

fn digit_button(ui: &mut Ui, label: &str) -> bool {
    styled_button(ui, label, GRAY, BTN)
}

fn wide_digit_button(ui: &mut Ui, label: &str, width: f32) -> bool {
    styled_button(ui, label, GRAY, width)
}

fn operator_button(ui: &mut Ui, label: &str) -> bool {
    styled_button(ui, label, palette::accent_now(), BTN)
}

fn action_button(ui: &mut Ui, label: &str) -> bool {
    styled_button(ui, label, DKGRAY, BTN)
}

fn styled_button(ui: &mut Ui, label: &str, fill: Color32, width: f32) -> bool {
    let text = RichText::new(label).size(22.0).monospace().color(WHITE);
    ui.add_sized([width, BTN], egui::Button::new(text).fill(fill))
        .clicked()
}

// ---------- tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    /// Walk a key sequence through the state machine.
    fn run(keys: &str) -> String {
        let mut calc = Calculator::default();
        for ch in keys.chars() {
            match ch {
                '0'..='9' => calc.press_digit(ch),
                '.' => calc.press_decimal(),
                '+' => calc.press_op(Op::Add),
                '-' => calc.press_op(Op::Sub),
                '*' => calc.press_op(Op::Mul),
                '/' => calc.press_op(Op::Div),
                '=' => calc.press_equals(),
                'C' => calc.press_clear(),
                'n' => calc.press_negate(),
                '%' => calc.press_percent(),
                'B' => calc.press_backspace(),
                ' ' => {}
                other => panic!("unknown key {other:?}"),
            }
        }
        calc.display
    }

    #[test]
    fn addition() {
        assert_eq!(run("5+3="), "8");
    }

    #[test]
    fn subtraction() {
        assert_eq!(run("10-7="), "3");
    }

    #[test]
    fn multiplication() {
        assert_eq!(run("6*7="), "42");
    }

    #[test]
    fn division() {
        assert_eq!(run("20/4="), "5");
    }

    #[test]
    fn division_direction_is_correct() {
        // Regression: original code evaluated right/left instead of left/right.
        assert_eq!(run("8/2="), "4");
    }

    #[test]
    fn division_by_zero_is_error_not_panic() {
        assert_eq!(run("8/0="), "Error");
    }

    #[test]
    fn decimals_are_supported() {
        // Regression: original process_calculation threw floats away.
        assert_eq!(run("0.5+0.25="), "0.75");
    }

    #[test]
    fn chained_operators_left_to_right() {
        assert_eq!(run("5+3+2="), "10");
        assert_eq!(run("5+3*2="), "16"); // no precedence, iOS-style
        assert_eq!(run("10-3+2="), "9");
    }

    #[test]
    fn zero_digit_does_not_double_up() {
        // Regression: starting at "0", pressing "0" used to produce "00".
        assert_eq!(run("00"), "0");
        assert_eq!(run("000"), "0");
    }

    #[test]
    fn decimal_only_once() {
        assert_eq!(run("1.2.3"), "1.23");
    }

    #[test]
    fn negate_toggles_sign() {
        assert_eq!(run("5n"), "-5");
        assert_eq!(run("5nn"), "5");
    }

    #[test]
    fn negate_on_zero_is_noop() {
        assert_eq!(run("n"), "0");
    }

    #[test]
    fn percent_divides_by_100() {
        assert_eq!(run("50%"), "0.5");
    }

    #[test]
    fn clear_resets_everything() {
        assert_eq!(run("5+3C"), "0");
        assert_eq!(run("5+3C7+2="), "9");
    }

    #[test]
    fn backspace_drops_last_digit() {
        assert_eq!(run("123B"), "12");
        assert_eq!(run("1B"), "0");
    }

    #[test]
    fn equals_starts_fresh_calculation_on_next_digit() {
        assert_eq!(run("5+3=7"), "7");
        assert_eq!(run("5+3=7+1="), "8");
    }

    #[test]
    fn equals_with_no_pending_op_is_noop() {
        assert_eq!(run("5="), "5");
    }

    #[test]
    fn operator_after_equals_uses_previous_result() {
        // 5+3=8, then *2 should give 16
        assert_eq!(run("5+3=*2="), "16");
    }

    #[test]
    fn double_operator_just_swaps_the_pending_one() {
        // User changes mind: 5 + then - then 3 = 2 (5 - 3)
        assert_eq!(run("5+-3="), "2");
    }

    #[test]
    fn error_latches_until_clear() {
        let mut calc = Calculator::default();
        for ch in "8/0=".chars() {
            match ch {
                '0'..='9' => calc.press_digit(ch),
                '/' => calc.press_op(Op::Div),
                '=' => calc.press_equals(),
                _ => {}
            }
        }
        assert_eq!(calc.display, "Error");
        calc.press_digit('5');
        assert_eq!(calc.display, "Error"); // digits ignored in error state
        calc.press_clear();
        calc.press_digit('5');
        assert_eq!(calc.display, "5");
    }

    #[test]
    fn format_large_whole_number_no_decimal() {
        assert_eq!(format_number(12345.0), "12345");
    }

    #[test]
    fn format_trims_trailing_zeros() {
        assert_eq!(format_number(1.5), "1.5");
        assert_eq!(format_number(1.25), "1.25");
    }
}
