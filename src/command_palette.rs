// ABOUTME: Cmd-K command palette - fuzzy search across windows, system
// ABOUTME: actions, and loaded notes; returns a single dispatched action.

use egui::{
    Context, Frame, Id, Key, KeyboardShortcut, Modal, Modifiers, RichText, ScrollArea, Stroke,
    StrokeKind, TextEdit, Ui,
};

/// One of the windows the palette can open. Mirrors the `*_is_open` flags
/// on `TemplateApp` — the dispatcher in `app.rs` translates these into
/// flag flips.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowKind {
    About,
    Calculator,
    Resume,
    DigitalGarden,
    Projects,
    Canvas,
    Workouts,
    Collections,
    BinaryClock,
    Bezier,
    PaletteStudio,
    Raycaster,
    WadViewer,
    AppEvents,
}

impl WindowKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::About => "About Me",
            Self::Calculator => "Calculator",
            Self::Resume => "Pseudo-Resumé",
            Self::DigitalGarden => "Digital Garden",
            Self::Projects => "Projects",
            Self::Canvas => "Canvas",
            Self::Workouts => "Workouts",
            Self::Collections => "Collections",
            Self::BinaryClock => "Binary Clock",
            Self::Bezier => "Bezier Playground",
            Self::PaletteStudio => "Palette Studio",
            Self::Raycaster => "Raycaster",
            Self::WadViewer => "WAD Viewer",
            Self::AppEvents => "App Events",
        }
    }

    fn group(self) -> &'static str {
        match self {
            Self::About | Self::Resume | Self::Projects => "identity",
            Self::Calculator
            | Self::BinaryClock
            | Self::Canvas
            | Self::Workouts
            | Self::Collections
            | Self::Bezier
            | Self::PaletteStudio
            | Self::Raycaster
            | Self::WadViewer => "tools",
            Self::DigitalGarden => "notes",
            Self::AppEvents => "system",
        }
    }
}

/// Built-in commands that don't open a window — system-level operations
/// surfaced through the palette so they're keyboard-discoverable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemAction {
    OrganizeWindows,
    ZoomIn,
    ZoomOut,
    ResetZoom,
}

impl SystemAction {
    fn label(self) -> &'static str {
        match self {
            Self::OrganizeWindows => "Organize windows",
            Self::ZoomIn => "Zoom in",
            Self::ZoomOut => "Zoom out",
            Self::ResetZoom => "Reset zoom",
        }
    }
}

/// One row in the palette. Built fresh each frame from current app state.
pub enum PaletteItem {
    Window(WindowKind),
    Note { id: String, title: String },
    System(SystemAction),
    Scheme(crate::palette::Scheme),
}

impl PaletteItem {
    fn search_text(&self) -> &str {
        match self {
            Self::Window(w) => w.label(),
            Self::Note { title, .. } => title,
            Self::System(s) => s.label(),
            Self::Scheme(s) => s.label(),
        }
    }

    fn group(&self) -> &'static str {
        match self {
            Self::Window(w) => w.group(),
            Self::Note { .. } => "note",
            Self::System(_) => "system",
            Self::Scheme(_) => "scheme",
        }
    }
}

/// The committed action returned to the dispatcher when the user picks an
/// item (Enter or click).
pub enum PaletteResult {
    OpenWindow(WindowKind),
    OpenNote(String),
    System(SystemAction),
    Scheme(crate::palette::Scheme),
}

#[derive(Default)]
pub struct CommandPalette {
    pub open: bool,
    query: String,
    /// Index into the *visible* (post-filter) result list.
    selected: usize,
    /// Set the frame the palette opens so we can request_focus() exactly
    /// once and not steal focus on subsequent frames.
    just_opened: bool,
}

/// Cmd+K on mac, Ctrl+K elsewhere. `Modifiers::COMMAND` is the
/// platform-aware "primary modifier" alias.
pub const TOGGLE_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::K);

/// Number-row shortcuts for jumping to the most-used windows. Cmd+1
/// through Cmd+5 toggle their respective windows. Indexed parallel to
/// the array returned by `WINDOW_SHORTCUTS`.
pub const WINDOW_SHORTCUTS: &[(KeyboardShortcut, WindowKind)] = &[
    (KeyboardShortcut::new(Modifiers::COMMAND, Key::Num1), WindowKind::DigitalGarden),
    (KeyboardShortcut::new(Modifiers::COMMAND, Key::Num2), WindowKind::Calculator),
    (KeyboardShortcut::new(Modifiers::COMMAND, Key::Num3), WindowKind::Canvas),
    (KeyboardShortcut::new(Modifiers::COMMAND, Key::Num4), WindowKind::Workouts),
    (KeyboardShortcut::new(Modifiers::COMMAND, Key::Num5), WindowKind::Projects),
];

impl CommandPalette {
    pub fn open(&mut self) {
        self.open = true;
        self.query.clear();
        self.selected = 0;
        self.just_opened = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    /// Render the palette modal (no-op when closed). Returns the action
    /// the user committed this frame, if any. The caller is responsible
    /// for actually dispatching it.
    pub fn ui(&mut self, ctx: &Context, items: Vec<PaletteItem>) -> Option<PaletteResult> {
        if !self.open {
            return None;
        }

        // Filter + score against the current query, keep the top 50 to
        // bound rendering cost on large note directories.
        let mut scored: Vec<(usize, i32)> = items
            .iter()
            .enumerate()
            .filter_map(|(i, item)| fuzzy_score(item.search_text(), &self.query).map(|s| (i, s)))
            .collect();
        scored.sort_by_key(|(_, s)| std::cmp::Reverse(*s));
        scored.truncate(50);
        let visible_count = scored.len();
        if visible_count == 0 {
            self.selected = 0;
        } else if self.selected >= visible_count {
            self.selected = visible_count - 1;
        }

        // Drive arrow-key nav *before* showing rows so the redraw reflects
        // the new selection in the same frame.
        ctx.input(|i| {
            if visible_count == 0 {
                return;
            }
            if i.key_pressed(Key::ArrowDown) {
                self.selected = (self.selected + 1) % visible_count;
            }
            if i.key_pressed(Key::ArrowUp) {
                self.selected = if self.selected == 0 {
                    visible_count - 1
                } else {
                    self.selected - 1
                };
            }
        });

        let mut result: Option<PaletteResult> = None;
        let response = Modal::new(Id::new("command_palette")).show(ctx, |ui| {
            ui.set_width(560.0);
            ui.set_max_height(420.0);

            // Search input - autofocus on first frame after open.
            let edit = ui.add(
                TextEdit::singleline(&mut self.query)
                    .hint_text("Search apps, actions, and notes…")
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Heading),
            );
            if self.just_opened {
                edit.request_focus();
                self.just_opened = false;
            }
            ui.add_space(4.0);

            // Result list.
            ScrollArea::vertical()
                .max_height(360.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if visible_count == 0 {
                        ui.add_space(20.0);
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new("No matches").weak());
                        });
                        return;
                    }
                    for (row_idx, (item_idx, _score)) in scored.iter().enumerate() {
                        let item = &items[*item_idx];
                        let is_selected = row_idx == self.selected;
                        let row = palette_row(ui, item, is_selected);
                        if row.clicked() {
                            result = Some(item_to_result(item));
                            self.selected = row_idx;
                        }
                        if is_selected {
                            row.scroll_to_me(None);
                        }
                    }
                });
        });

        // Enter commits the selected row. Run after the modal so the
        // user's typing in the input doesn't double-dispatch on the
        // first Enter — egui's TextEdit doesn't consume Enter by default
        // for singleline edits, which is what we want here.
        if result.is_none() && visible_count > 0 {
            ctx.input(|i| {
                if i.key_pressed(Key::Enter) {
                    if let Some((item_idx, _)) = scored.get(self.selected) {
                        result = Some(item_to_result(&items[*item_idx]));
                    }
                }
            });
        }

        if response.should_close() || result.is_some() {
            self.open = false;
        }

        result
    }
}

fn item_to_result(item: &PaletteItem) -> PaletteResult {
    match item {
        PaletteItem::Window(w) => PaletteResult::OpenWindow(*w),
        PaletteItem::Note { id, .. } => PaletteResult::OpenNote(id.clone()),
        PaletteItem::System(s) => PaletteResult::System(*s),
        PaletteItem::Scheme(s) => PaletteResult::Scheme(*s),
    }
}

fn palette_row(ui: &mut Ui, item: &PaletteItem, selected: bool) -> egui::Response {
    let visuals = ui.visuals();
    let bg = if selected {
        visuals.selection.bg_fill
    } else {
        visuals.faint_bg_color
    };
    let frame = Frame::new()
        .fill(bg)
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .stroke(if selected {
            Stroke::new(1.0, visuals.selection.stroke.color)
        } else {
            Stroke::NONE
        });

    let response = ui
        .scope_builder(
            egui::UiBuilder::new().sense(egui::Sense::click()),
            |ui| {
                frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.set_width(ui.available_width());
                        ui.label(item.search_text());
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.label(
                                    RichText::new(item.group()).small().weak(),
                                );
                            },
                        );
                    });
                });
            },
        )
        .response;
    // Layered under the frame: the frame paints the bg, the response
    // captures the click. `StrokeKind::Inside` is unused here but kept
    // imported to keep the prelude small if we later add per-row borders.
    let _ = StrokeKind::Inside;
    response
}

/// Tiny case-insensitive scorer: empty query → score 0 (everything
/// passes); contiguous substring match → high score with earlier
/// positions winning; subsequence match → lower score weighted by gap
/// size; no match → None.
fn fuzzy_score(haystack: &str, needle: &str) -> Option<i32> {
    if needle.is_empty() {
        return Some(0);
    }
    let h = haystack.to_lowercase();
    let n = needle.to_lowercase();
    if let Some(pos) = h.find(&n) {
        // Substring beats subsequence by a wide margin; earlier positions
        // beat later; exact-prefix gets a bonus.
        let prefix_bonus = if pos == 0 { 200 } else { 0 };
        return Some(1000 - pos as i32 + prefix_bonus);
    }
    // Subsequence: every char in `n` must appear in `h`, in order.
    let mut h_chars = h.chars().peekable();
    let mut score: i32 = 0;
    let mut gap_since_match: i32 = 0;
    for nc in n.chars() {
        loop {
            let hc = h_chars.next()?;
            if hc == nc {
                score += 10 - gap_since_match.min(9);
                gap_since_match = 0;
                break;
            }
            gap_since_match += 1;
        }
    }
    Some(score)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_scores_zero() {
        assert_eq!(fuzzy_score("anything", ""), Some(0));
    }

    #[test]
    fn substring_beats_subsequence() {
        let sub = fuzzy_score("calculator", "calc").unwrap();
        let seq = fuzzy_score("c-a-l-c", "calc").unwrap();
        assert!(sub > seq);
    }

    #[test]
    fn prefix_beats_inner_substring() {
        let prefix = fuzzy_score("calculator", "calc").unwrap();
        let inner = fuzzy_score("recalculate", "calc").unwrap();
        assert!(prefix > inner);
    }

    #[test]
    fn case_insensitive() {
        assert!(fuzzy_score("Calculator", "CALC").is_some());
    }

    #[test]
    fn no_match_returns_none() {
        assert!(fuzzy_score("calculator", "xyz").is_none());
    }
}
