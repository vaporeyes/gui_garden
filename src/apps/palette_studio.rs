// ABOUTME: Live editor for the active Poline color scheme - drag anchor
// ABOUTME: points on the disk, tweak per-axis position fns, apply.
//
// The wheel is the polar HSL disk (hue around the circumference,
// lightness as the radial distance). Each anchor is a draggable point
// on the disk; saturation lives on a per-anchor slider since it's the
// orthogonal Z axis. Below the wheel sits the resulting palette as
// swatches; the right-side panel exposes points-per-segment, the three
// position-function dropdowns, the closed-loop toggle, and an "Apply"
// button that writes the edited Poline into the global custom slot
// and switches the active scheme to `Custom`.

use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::palette::{self, Hsl, PositionFn, Poline, Scheme};

#[derive(Default)]
pub struct PaletteStudio {
    /// In-flight Poline being edited. Hydrated from the active scheme
    /// the first time the editor renders.
    poline: Option<Poline>,
    /// Index of the anchor currently being dragged, if any.
    dragging: Option<usize>,
}

impl PaletteStudio {
    pub fn ui(&mut self, ui: &mut Ui) {
        // Lazy-hydrate the editor state from whichever Poline is active
        // right now. First open after launch picks up whatever scheme
        // the user persisted.
        if self.poline.is_none() {
            self.poline = Some(palette::active_scheme().poline());
        }
        // Split the borrows: `poline` and `dragging` both live on `self`,
        // and the closure captures `self` mutably otherwise. Destructure
        // up front so the closure body only sees disjoint refs.
        let Self { poline, dragging } = self;
        let poline = poline.as_mut().expect("hydrated above");

        ui.horizontal_top(|ui| {
            // Left: the polar wheel.
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Color wheel").small().weak());
                draw_wheel(ui, poline, dragging);
            });

            ui.add_space(12.0);

            // Right: settings.
            ui.vertical(|ui| {
                ui.set_min_width(260.0);
                ui.label(egui::RichText::new("Anchors").small().weak());
                let mut delete_at: Option<usize> = None;
                for (i, anchor) in poline.anchors.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        let swatch = anchor.to_color32();
                        let (rect, _) =
                            ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                        ui.painter().rect_filled(rect, 3.0, swatch);
                        ui.label(format!("{}", i + 1));
                        ui.add(
                            egui::Slider::new(&mut anchor.s, 0.0..=1.0)
                                .text("sat")
                                .clamping(egui::SliderClamping::Always),
                        );
                        if ui.small_button("✕").on_hover_text("Remove anchor").clicked() {
                            delete_at = Some(i);
                        }
                    });
                }
                if let Some(i) = delete_at {
                    if poline.anchors.len() > 2 {
                        poline.anchors.remove(i);
                    }
                }

                ui.horizontal(|ui| {
                    if ui.button("➕ Add anchor").clicked() {
                        // Insert a midpoint between the last two anchors
                        // so the new anchor lands somewhere meaningful.
                        let last = poline.anchors.last().copied();
                        let prev = poline
                            .anchors
                            .get(poline.anchors.len().saturating_sub(2))
                            .copied();
                        let new = match (last, prev) {
                            (Some(a), Some(b)) => Hsl::new(
                                ((a.h + b.h) * 0.5).rem_euclid(360.0),
                                (a.s + b.s) * 0.5,
                                (a.l + b.l) * 0.5,
                            ),
                            (Some(a), _) => Hsl::new((a.h + 60.0) % 360.0, a.s, a.l),
                            _ => Hsl::new(40.0, 0.6, 0.5),
                        };
                        poline.anchors.push(new);
                    }
                });

                ui.separator();
                ui.label(egui::RichText::new("Curve").small().weak());
                ui.add(
                    egui::Slider::new(&mut poline.points_per_segment, 2..=12)
                        .text("points / segment"),
                );
                ui.checkbox(&mut poline.closed, "Closed loop");
                position_fn_combo(ui, "fn x (hue)", &mut poline.fn_x);
                position_fn_combo(ui, "fn y (hue)", &mut poline.fn_y);
                position_fn_combo(ui, "fn z (sat)", &mut poline.fn_z);

                ui.separator();
                ui.horizontal(|ui| {
                    if ui
                        .button("Apply as Custom")
                        .on_hover_text(
                            "Write this Poline to the custom slot and switch the active scheme to Custom",
                        )
                        .clicked()
                    {
                        palette::set_custom_poline(poline.clone());
                        palette::set_active_scheme(Scheme::Custom);
                        ui.ctx().request_repaint();
                    }
                    if ui.button("Reset from active").clicked() {
                        // Defer the reset until after this borrow ends.
                        *poline = palette::active_scheme().poline();
                    }
                });
            });
        });

        ui.add_space(8.0);
        ui.separator();
        ui.label(egui::RichText::new("Generated palette").small().weak());
        let palette_colors = poline.colors();
        draw_swatches(ui, &palette_colors);
    }
}

fn draw_wheel(ui: &mut Ui, poline: &mut Poline, dragging: &mut Option<usize>) {
    const WHEEL_SIZE: f32 = 280.0;
    let (rect, response) =
        ui.allocate_exact_size(Vec2::splat(WHEEL_SIZE), Sense::click_and_drag());
    let center = rect.center();
    let radius = WHEEL_SIZE * 0.5 - 6.0;

    // Faint disk backdrop so the editing surface is visually demarcated.
    ui.painter()
        .circle_filled(center, radius, ui.visuals().extreme_bg_color);
    ui.painter()
        .circle_stroke(center, radius, Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color));

    // Cardinal hue ticks (0/90/180/270°) so the user has rotational
    // reference even without a full color-mapped backdrop. Cheap.
    for deg in [0u32, 90, 180, 270] {
        let rad = (deg as f32).to_radians();
        let p = Pos2::new(
            center.x + radius * rad.cos(),
            center.y + radius * rad.sin(),
        );
        ui.painter().circle_filled(p, 2.0, ui.visuals().weak_text_color());
    }

    // Drag handling: pick the closest anchor on press; track drag deltas.
    if response.drag_started() {
        if let Some(p) = response.interact_pointer_pos() {
            let mut best = None;
            for (i, a) in poline.anchors.iter().enumerate() {
                let pos = anchor_screen_pos(*a, center, radius);
                let d = pos.distance(p);
                if d < 14.0 && best.is_none_or(|(_, bd): (usize, f32)| d < bd) {
                    best = Some((i, d));
                }
            }
            *dragging = best.map(|(i, _)| i);
        }
    }
    if response.drag_stopped() {
        *dragging = None;
    }
    if response.dragged() {
        if let (Some(idx), Some(p)) = (*dragging, response.interact_pointer_pos()) {
            // Convert screen pos back into HSL (h, l) on the disk; clamp
            // to circle edge so anchors stay inside the wheel.
            let mut dx = p.x - center.x;
            let mut dy = p.y - center.y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist > radius {
                let s = radius / dist;
                dx *= s;
                dy *= s;
            }
            let mut h = dy.atan2(dx).to_degrees();
            if h < 0.0 {
                h += 360.0;
            }
            let l = ((dx * dx + dy * dy).sqrt() / radius).clamp(0.0, 1.0);
            if let Some(anchor) = poline.anchors.get_mut(idx) {
                anchor.h = h;
                anchor.l = l;
            }
        }
    }

    // Generated curve trail — paint each generated color as a small dot
    // at its (h, l) on the disk so users see the actual interpolated
    // path between anchors.
    for (i, color) in poline.colors().iter().enumerate() {
        // Approximate Hsl from the Color32 isn't worth it; instead we
        // re-walk the same chain mathematically. Skip for cost; just
        // dot-mark anchors below.
        let _ = (i, color);
    }

    // Anchor dots, drawn last so they sit above ticks.
    for (i, a) in poline.anchors.iter().enumerate() {
        let pos = anchor_screen_pos(*a, center, radius);
        let color = a.to_color32();
        ui.painter().circle_filled(pos, 9.0, color);
        ui.painter().circle_stroke(
            pos,
            9.0,
            Stroke::new(2.0, ui.visuals().strong_text_color()),
        );
        ui.painter().text(
            pos,
            egui::Align2::CENTER_CENTER,
            format!("{}", i + 1),
            egui::FontId::monospace(10.0),
            ui.visuals().extreme_bg_color,
        );
    }
}

fn anchor_screen_pos(a: Hsl, center: Pos2, radius: f32) -> Pos2 {
    let rad = a.h.to_radians();
    let r = a.l * radius;
    Pos2::new(center.x + r * rad.cos(), center.y + r * rad.sin())
}

fn position_fn_combo(ui: &mut Ui, label: &str, current: &mut PositionFn) {
    egui::ComboBox::from_label(label)
        .selected_text(position_fn_label(*current))
        .show_ui(ui, |ui| {
            for f in [
                PositionFn::Linear,
                PositionFn::Sinusoidal,
                PositionFn::ASinusoidal,
                PositionFn::Exponential,
                PositionFn::Quadratic,
                PositionFn::Cubic,
                PositionFn::Quartic,
                PositionFn::Arc,
                PositionFn::SmoothStep,
            ] {
                ui.selectable_value(current, f, position_fn_label(f));
            }
        });
}

fn position_fn_label(f: PositionFn) -> &'static str {
    match f {
        PositionFn::Linear => "Linear",
        PositionFn::Sinusoidal => "Sinusoidal",
        PositionFn::ASinusoidal => "ASinusoidal",
        PositionFn::Exponential => "Exponential",
        PositionFn::Quadratic => "Quadratic",
        PositionFn::Cubic => "Cubic",
        PositionFn::Quartic => "Quartic",
        PositionFn::Arc => "Arc",
        PositionFn::SmoothStep => "SmoothStep",
    }
}

fn draw_swatches(ui: &mut Ui, colors: &[Color32]) {
    if colors.is_empty() {
        return;
    }
    let total_width = ui.available_width().min(560.0);
    let height = 36.0;
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(total_width, height), Sense::hover());
    let n = colors.len() as f32;
    let step = rect.width() / n;
    for (i, c) in colors.iter().enumerate() {
        let x0 = rect.left() + step * i as f32;
        let cell = Rect::from_min_max(
            Pos2::new(x0, rect.top()),
            Pos2::new(x0 + step, rect.bottom()),
        );
        ui.painter().rect_filled(cell, 0.0, *c);
    }
    ui.painter().rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
        egui::StrokeKind::Outside,
    );
}
