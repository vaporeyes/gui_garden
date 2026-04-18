// Workouts calendar heatmap.
//
// Reads an astro-blog-style `workouts.json` and renders a GitHub-style
// day-density grid (7 rows × N weeks). Hover reveals the session count
// and names for that day. Optional total / month summaries are shown in
// the header strip.

// File loading is native-only; on wasm the stub `pick_and_load` returns
// an error without touching the filesystem, so most parse/flatten helpers
// never run. Silence those warnings rather than cfg-gating every struct.
#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

use chrono::{Datelike, NaiveDate, Weekday};
use egui::{Color32, Pos2, Rect, Sense, Ui, Vec2};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::palette;

#[derive(Debug, Deserialize, Clone)]
struct WorkoutFile {
    #[serde(rename = "totalSessions")]
    total_sessions: Option<u32>,
    #[serde(default)]
    months: Vec<WorkoutMonth>,
}

#[derive(Debug, Deserialize, Clone)]
struct WorkoutMonth {
    #[serde(default)]
    sessions: Vec<WorkoutSession>,
}

#[derive(Debug, Deserialize, Clone)]
struct WorkoutSession {
    date: String,
    name: Option<String>,
    duration: Option<String>,
    #[serde(default)]
    exercises: Vec<WorkoutExercise>,
}

#[derive(Debug, Deserialize, Clone)]
struct WorkoutExercise {
    name: String,
    #[serde(default)]
    sets: Vec<WorkoutSet>,
}

#[derive(Debug, Deserialize, Clone, Default)]
struct WorkoutSet {
    #[serde(default)]
    weight: f32,
    #[serde(default)]
    reps: u32,
    /// Rate of Perceived Exertion. Parsed from the source JSON but not yet
    /// surfaced in the detail view — reserved for a future "effort arc"
    /// visualization per session.
    #[serde(default)]
    #[allow(dead_code)]
    rpe: Option<f32>,
}

#[derive(Debug, Clone)]
struct SessionDetail {
    name: String,
    duration: Option<String>,
    exercises: Vec<WorkoutExercise>,
}

#[derive(Debug, Clone)]
struct DayEntry {
    #[allow(dead_code)]
    date: NaiveDate,
    sessions: Vec<SessionDetail>,
}

pub struct Workouts {
    loaded: Option<(PathBuf, WorkoutFile)>,
    days: BTreeMap<NaiveDate, DayEntry>,
    /// `(date, exercise) → true` when that session's top set set a new
    /// all-time max weight for that exercise. Computed once at load time.
    prs: std::collections::HashSet<(NaiveDate, String)>,
    error: Option<String>,
    /// Selected day — click a cell to pin it; the detail panel shows its
    /// exercises and set volumes until another cell is clicked.
    selected: Option<NaiveDate>,
}

impl Default for Workouts {
    fn default() -> Self {
        Self {
            loaded: None,
            days: BTreeMap::new(),
            prs: std::collections::HashSet::new(),
            error: None,
            selected: None,
        }
    }
}

impl Workouts {
    #[cfg(not(target_arch = "wasm32"))]
    fn pick_and_load(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("workouts.json", &["json"])
            .set_title("Load a workouts.json")
            .pick_file()
        else {
            return;
        };
        self.load_path(path);
    }

    #[cfg(target_arch = "wasm32")]
    fn pick_and_load(&mut self) {
        self.error = Some("File picker unavailable on web.".into());
    }

    /// Load a workouts JSON file from an explicit path. Used at startup to
    /// auto-rehydrate the last file the user picked.
    pub fn load_from_path<P: Into<PathBuf>>(&mut self, path: P) {
        self.load_path(path.into());
    }

    /// Path of the currently-loaded file (empty if nothing is loaded).
    pub fn loaded_path(&self) -> Option<&Path> {
        self.loaded.as_ref().map(|(p, _)| p.as_path())
    }

    fn load_path(&mut self, path: PathBuf) {
        match std::fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str::<WorkoutFile>(&content) {
                Ok(file) => {
                    self.days = flatten_days(&file);
                    self.prs = compute_prs(&self.days);
                    self.loaded = Some((path, file));
                    self.error = None;
                }
                Err(e) => self.error = Some(format!("parse error: {}", e)),
            },
            Err(e) => self.error = Some(format!("read error: {}", e)),
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let accent = palette::accent_now();

        ui.horizontal(|ui| {
            if ui.button("📁 Load workouts.json…").clicked() {
                self.pick_and_load();
            }
            if let Some((_, file)) = &self.loaded {
                let total = file.total_sessions.unwrap_or(0);
                ui.label(
                    egui::RichText::new(format!(
                        "{} sessions across {} days",
                        total,
                        self.days.len()
                    ))
                    .weak(),
                );
            }
        });
        if let Some(err) = &self.error {
            ui.colored_label(Color32::from_rgb(220, 80, 80), err);
        }
        ui.separator();

        if self.days.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new(
                        "No workouts loaded. Try ~/dev/projects/astro-blog/src/data/workouts.json",
                    )
                    .weak(),
                );
            });
            return;
        }

        // Heatmap can easily be wider than the window (4 years ≈ 210 weeks),
        // so wrap it in a horizontal scroll area. Vertical auto-shrinks so
        // the detail panel below stays flush against the grid.
        //
        // `id_salt` is critical: the detail panel below uses its own vertical
        // ScrollArea, and without explicit salts the two sibling ScrollAreas
        // derive identical auto-IDs from the shared parent ui — which cascades
        // into duplicate-ID warnings for every inner widget (viewport, scrollbar).
        egui::ScrollArea::horizontal()
            .id_salt("workouts_heatmap_scroll")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                self.render_heatmap(ui, accent);
            });

        // Density legend. Lives *outside* the horizontal scroll so it's
        // always visible regardless of where the user has scrolled the
        // grid to.
        ui.add_space(4.0);
        render_legend(ui, accent);
        if let Some(date) = self.selected {
            if let Some(entry) = self.days.get(&date).cloned() {
                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);
                render_detail(ui, date, &entry, accent, &self.prs);
            }
        }
    }

    fn render_heatmap(&mut self, ui: &mut Ui, accent: Color32) {
        // Build a contiguous weekly grid from the first Sunday ≤ min_date
        // to the last Saturday ≥ max_date. Empty days get rendered as
        // faint squares so the density contrast reads clearly.
        let (Some(&first), Some(&last)) =
            (self.days.keys().next(), self.days.keys().last())
        else {
            return;
        };

        let start = align_to_sunday(first);
        let end = align_to_saturday(last);
        let total_days = (end - start).num_days() as usize + 1;
        let weeks = (total_days + 6) / 7;

        let max_sessions = self
            .days
            .values()
            .map(|d| d.sessions.len())
            .max()
            .unwrap_or(1) as f32;

        // Geometry
        let cell_size = 12.0;
        let cell_gap = 2.0;
        let label_width = 40.0; // room for "Mon" etc. on the left
        let header_height = 18.0; // month labels on top

        let grid_w = weeks as f32 * (cell_size + cell_gap);
        let grid_h = 7.0 * (cell_size + cell_gap);

        let total_size = Vec2::new(label_width + grid_w, header_height + grid_h);
        let (response, painter) = ui.allocate_painter(total_size, Sense::click_and_drag());
        let origin = response.rect.min;

        let grid_origin = origin + Vec2::new(label_width, header_height);

        // Month labels along the top — mark the week that starts each month.
        let mut last_label_month = 0u32;
        for w in 0..weeks {
            let day = start + chrono::Duration::days(w as i64 * 7);
            if day.month() != last_label_month {
                last_label_month = day.month();
                let x = grid_origin.x + w as f32 * (cell_size + cell_gap);
                let y = origin.y;
                painter.text(
                    Pos2::new(x, y),
                    egui::Align2::LEFT_TOP,
                    month_short(day.month()),
                    egui::FontId::proportional(10.0),
                    ui.visuals().weak_text_color(),
                );
            }
        }

        // Day-of-week labels on the left (Mon / Wed / Fri to avoid crowding).
        let dow_labels = [(1, "Mon"), (3, "Wed"), (5, "Fri")];
        for (row, label) in dow_labels {
            let y = grid_origin.y + row as f32 * (cell_size + cell_gap) + cell_size / 2.0;
            painter.text(
                Pos2::new(origin.x, y),
                egui::Align2::LEFT_CENTER,
                label,
                egui::FontId::proportional(10.0),
                ui.visuals().weak_text_color(),
            );
        }

        // Cells
        let empty_fill = ui.visuals().faint_bg_color;
        let hover = response.hover_pos();
        let mut tooltip: Option<(NaiveDate, &DayEntry)> = None;
        let mut just_clicked: Option<NaiveDate> = None;

        for i in 0..total_days {
            let date = start + chrono::Duration::days(i as i64);
            let col = i / 7;
            let row = weekday_index(date.weekday());
            let x = grid_origin.x + col as f32 * (cell_size + cell_gap);
            let y = grid_origin.y + row as f32 * (cell_size + cell_gap);
            let rect = Rect::from_min_size(Pos2::new(x, y), Vec2::splat(cell_size));

            let entry = self.days.get(&date);
            let fill = match entry {
                Some(entry) => {
                    let density = (entry.sessions.len() as f32 / max_sessions).clamp(0.0, 1.0);
                    accent.linear_multiply(0.25 + 0.75 * density)
                }
                None => empty_fill,
            };
            painter.rect_filled(rect, 2.0, fill);

            // Draw a ring around the pinned cell.
            if self.selected == Some(date) {
                painter.rect_stroke(
                    rect.expand(1.0),
                    2.0,
                    egui::Stroke::new(1.5, accent),
                    egui::StrokeKind::Outside,
                );
            }

            if let Some(hp) = hover {
                if rect.contains(hp) {
                    if let Some(entry) = entry {
                        tooltip = Some((date, entry));
                        if response.clicked() {
                            just_clicked = Some(date);
                        }
                    }
                }
            }
        }

        if let Some((date, entry)) = tooltip {
            let names: Vec<&str> = entry.sessions.iter().map(|s| s.name.as_str()).collect();
            let text = format!(
                "{}\n{} session{}\n{}\n\n(click to drill in)",
                date.format("%A, %B %-d, %Y"),
                entry.sessions.len(),
                if entry.sessions.len() == 1 { "" } else { "s" },
                names.join("\n"),
            );
            response.clone().on_hover_text(text);
        }

        if let Some(date) = just_clicked {
            self.selected = Some(date);
        }
    }
}

/// Small "less ↔ more" density ramp below the heatmap. 5 representative
/// swatches between 0 and 1 density, sharing the same mapping used by
/// `render_heatmap` for real cells.
fn render_legend(ui: &mut Ui, accent: Color32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.label(egui::RichText::new("less").small().weak());
        ui.add_space(6.0);
        let cell = 10.0;
        for i in 0..5 {
            let density = 0.05 + 0.23 * i as f32;
            let (rect, _) =
                ui.allocate_exact_size(Vec2::splat(cell), egui::Sense::hover());
            ui.painter().rect_filled(
                rect,
                2.0,
                accent.linear_multiply(0.25 + 0.75 * density),
            );
        }
        ui.add_space(6.0);
        ui.label(egui::RichText::new("more").small().weak());
    });
}

fn render_detail(
    ui: &mut Ui,
    date: NaiveDate,
    entry: &DayEntry,
    accent: Color32,
    prs: &std::collections::HashSet<(NaiveDate, String)>,
) {
    ui.label(
        egui::RichText::new(date.format("%A, %B %-d, %Y").to_string())
            .size(18.0)
            .strong()
            .color(accent),
    );
    ui.add_space(6.0);

    egui::ScrollArea::vertical()
        .id_salt("workouts_detail_scroll")
        .auto_shrink([false, false])
        .max_height(220.0)
        .show(ui, |ui| {
            for session in &entry.sessions {
                egui::Frame::NONE
                    .fill(ui.visuals().faint_bg_color)
                    .inner_margin(egui::Margin::same(8))
                    .corner_radius(egui::CornerRadius::same(4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(&session.name).strong());
                            if let Some(d) = &session.duration {
                                ui.label(egui::RichText::new(format!("· {}", d)).weak());
                            }
                        });
                        ui.add_space(2.0);
                        for ex in &session.exercises {
                            render_exercise_row(ui, ex, date, accent, prs);
                        }
                    });
                ui.add_space(6.0);
            }
        });
}

fn render_exercise_row(
    ui: &mut Ui,
    ex: &WorkoutExercise,
    date: NaiveDate,
    accent: Color32,
    prs: &std::collections::HashSet<(NaiveDate, String)>,
) {
    let total_volume: f32 = ex.sets.iter().map(|s| s.weight * s.reps as f32).sum();
    let is_pr = prs.contains(&(date, ex.name.clone()));

    ui.horizontal(|ui| {
        let line = if ex.sets.is_empty() {
            ex.name.clone()
        } else if total_volume > 0.0 {
            format!(
                "{}  —  {} sets, {:.0} lb × reps",
                ex.name,
                ex.sets.len(),
                total_volume
            )
        } else {
            format!("{}  —  {} sets", ex.name, ex.sets.len())
        };
        let mut text = egui::RichText::new(line).small();
        if is_pr {
            text = text.color(accent).strong();
        }
        ui.label(text);
        if is_pr {
            ui.label(
                egui::RichText::new("★")
                    .small()
                    .color(accent)
                    .strong(),
            )
            .on_hover_text("All-time best weight for this exercise");
        }

        // RPE sparkline — compact per-set effort arc on the right.
        render_rpe_sparkline(ui, &ex.sets, accent);
    });
}

/// Tiny sparkline showing RPE across sets. Skipped entirely if no set
/// has an RPE — keeps the layout tight for bodyweight/cardio exercises.
fn render_rpe_sparkline(ui: &mut Ui, sets: &[WorkoutSet], accent: Color32) {
    let rpes: Vec<f32> = sets.iter().filter_map(|s| s.rpe).collect();
    if rpes.is_empty() {
        return;
    }
    // Reserve a small canvas on the right of the row.
    let h = 14.0;
    let w = 3.0 * rpes.len() as f32 + 2.0 * (rpes.len().saturating_sub(1)) as f32;
    let w = w.max(20.0);
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::hover());

    let min_rpe = 5.0_f32; // RPE below 5 is effectively "warmup" in Strong
    let max_rpe = 10.0_f32;
    let painter = ui.painter();
    let baseline_y = rect.bottom() - 1.0;
    let top_y = rect.top() + 1.0;
    let step = if rpes.len() > 1 {
        (rect.width() - 3.0) / (rpes.len() - 1) as f32
    } else {
        0.0
    };

    // Draw a faint baseline.
    painter.line_segment(
        [
            egui::Pos2::new(rect.left(), baseline_y),
            egui::Pos2::new(rect.right(), baseline_y),
        ],
        egui::Stroke::new(0.5, ui.visuals().weak_text_color()),
    );

    // Then the RPE points, colored by intensity.
    for (i, rpe) in rpes.iter().enumerate() {
        let x = rect.left() + 1.5 + step * i as f32;
        let t = ((rpe - min_rpe) / (max_rpe - min_rpe)).clamp(0.0, 1.0);
        let y = baseline_y + (top_y - baseline_y) * t;
        let color = accent.linear_multiply(0.4 + 0.6 * t);
        painter.circle_filled(egui::Pos2::new(x, y), 1.5, color);
        if i > 0 {
            let prev_rpe = rpes[i - 1];
            let prev_t = ((prev_rpe - min_rpe) / (max_rpe - min_rpe)).clamp(0.0, 1.0);
            let prev_x = rect.left() + 1.5 + step * (i - 1) as f32;
            let prev_y = baseline_y + (top_y - baseline_y) * prev_t;
            painter.line_segment(
                [
                    egui::Pos2::new(prev_x, prev_y),
                    egui::Pos2::new(x, y),
                ],
                egui::Stroke::new(1.0, accent.linear_multiply(0.5)),
            );
        }
    }

    let tip = rpes
        .iter()
        .map(|r| format!("{:.1}", r))
        .collect::<Vec<_>>()
        .join(" · ");
    response.on_hover_text(format!("RPE: {}", tip));
}

/// Strong-style "personal record" calculation. For each exercise, walk
/// the days chronologically and flag each date that strictly exceeds the
/// running max weight (across any set on that day) for that exercise.
fn compute_prs(
    days: &BTreeMap<NaiveDate, DayEntry>,
) -> std::collections::HashSet<(NaiveDate, String)> {
    let mut prs = std::collections::HashSet::new();
    let mut running_max: std::collections::HashMap<String, f32> =
        std::collections::HashMap::new();

    for (date, entry) in days.iter() {
        // Max weight seen for each exercise this day.
        let mut today_max: std::collections::HashMap<String, f32> =
            std::collections::HashMap::new();
        for session in &entry.sessions {
            for ex in &session.exercises {
                let m = ex.sets.iter().map(|s| s.weight).fold(0.0_f32, f32::max);
                let cur = today_max.entry(ex.name.clone()).or_insert(0.0);
                if m > *cur {
                    *cur = m;
                }
            }
        }
        // Compare against the running all-time max.
        for (name, today) in today_max {
            if today <= 0.0 {
                continue; // bodyweight / cardio — skip
            }
            let best = running_max.entry(name.clone()).or_insert(0.0);
            if today > *best {
                *best = today;
                prs.insert((*date, name));
            }
        }
    }
    prs
}

fn flatten_days(file: &WorkoutFile) -> BTreeMap<NaiveDate, DayEntry> {
    let mut out: BTreeMap<NaiveDate, DayEntry> = BTreeMap::new();
    for month in &file.months {
        for session in &month.sessions {
            // Dates look like "2022-05-11 04:20:50" — take the first 10 chars.
            let date_str = session.date.get(..10).unwrap_or(&session.date);
            let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") else {
                continue;
            };
            let entry = out.entry(date).or_insert_with(|| DayEntry {
                date,
                sessions: Vec::new(),
            });
            entry.sessions.push(SessionDetail {
                name: session.name.clone().unwrap_or_else(|| "session".to_string()),
                duration: session.duration.clone(),
                exercises: session.exercises.clone(),
            });
        }
    }
    out
}

fn align_to_sunday(d: NaiveDate) -> NaiveDate {
    let back = weekday_index(d.weekday());
    d - chrono::Duration::days(back as i64)
}

fn align_to_saturday(d: NaiveDate) -> NaiveDate {
    let fwd = 6 - weekday_index(d.weekday());
    d + chrono::Duration::days(fwd as i64)
}

/// Sun=0 … Sat=6, matching the astro-blog heatmap orientation.
fn weekday_index(w: Weekday) -> usize {
    match w {
        Weekday::Sun => 0,
        Weekday::Mon => 1,
        Weekday::Tue => 2,
        Weekday::Wed => 3,
        Weekday::Thu => 4,
        Weekday::Fri => 5,
        Weekday::Sat => 6,
    }
}

fn month_short(m: u32) -> &'static str {
    match m {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "",
    }
}

// ---------- tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    fn session(date: &str, name: &str) -> WorkoutSession {
        WorkoutSession {
            date: date.into(),
            name: Some(name.into()),
            duration: None,
            exercises: Vec::new(),
        }
    }

    #[test]
    fn flatten_days_groups_by_date() {
        let file = WorkoutFile {
            total_sessions: Some(3),
            months: vec![WorkoutMonth {
                sessions: vec![
                    session("2022-05-11 04:20:50", "ppl"),
                    session("2022-05-11 18:00:00", "cardio"),
                    session("2022-05-12 08:00:00", "legs"),
                ],
            }],
        };
        let days = flatten_days(&file);
        assert_eq!(days.len(), 2);
        let may11 = NaiveDate::from_ymd_opt(2022, 5, 11).unwrap();
        assert_eq!(days.get(&may11).unwrap().sessions.len(), 2);
    }

    #[test]
    fn flatten_days_skips_bad_dates() {
        let file = WorkoutFile {
            total_sessions: None,
            months: vec![WorkoutMonth {
                sessions: vec![WorkoutSession {
                    date: "garbage".into(),
                    name: None,
                    duration: None,
                    exercises: Vec::new(),
                }],
            }],
        };
        assert!(flatten_days(&file).is_empty());
    }

    #[test]
    fn flatten_days_preserves_exercise_details() {
        let file = WorkoutFile {
            total_sessions: Some(1),
            months: vec![WorkoutMonth {
                sessions: vec![WorkoutSession {
                    date: "2026-04-17 06:00:00".into(),
                    name: Some("upper".into()),
                    duration: Some("45m".into()),
                    exercises: vec![WorkoutExercise {
                        name: "Bench Press".into(),
                        sets: vec![
                            WorkoutSet { weight: 185.0, reps: 5, rpe: Some(8.0) },
                            WorkoutSet { weight: 185.0, reps: 5, rpe: Some(8.5) },
                        ],
                    }],
                }],
            }],
        };
        let days = flatten_days(&file);
        let date = NaiveDate::from_ymd_opt(2026, 4, 17).unwrap();
        let day = days.get(&date).unwrap();
        assert_eq!(day.sessions.len(), 1);
        let session = &day.sessions[0];
        assert_eq!(session.name, "upper");
        assert_eq!(session.duration.as_deref(), Some("45m"));
        assert_eq!(session.exercises.len(), 1);
        assert_eq!(session.exercises[0].sets.len(), 2);
    }

    #[test]
    fn sunday_alignment() {
        // 2022-05-11 was a Wednesday → back to 2022-05-08 (Sunday).
        let d = NaiveDate::from_ymd_opt(2022, 5, 11).unwrap();
        assert_eq!(
            align_to_sunday(d),
            NaiveDate::from_ymd_opt(2022, 5, 8).unwrap()
        );
    }

    #[test]
    fn saturday_alignment() {
        let d = NaiveDate::from_ymd_opt(2022, 5, 11).unwrap();
        assert_eq!(
            align_to_saturday(d),
            NaiveDate::from_ymd_opt(2022, 5, 14).unwrap()
        );
    }
}
