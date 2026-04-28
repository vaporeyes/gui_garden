// Rust port of Poline's polar-interpolation palette engine, used
// across the app (theme, fractal clock, calculator, graph view) to
// keep a coherent accent palette per scheme.
//
// The polar XYZ mapping (the trick that gives Poline its "non-linear"
// feel) lifts each HSL color onto a unit-circle disk so that
// straight-line interpolation in XYZ produces the smooth, swooping
// gradients Poline is known for. See `hsl_to_xyz` / `xyz_to_hsl`.
//
// Multiple named schemes (`Scheme::*`) are baked in. A runtime atomic
// (`ACTIVE_SCHEME`) selects which one `accent_now()` and
// `anchors_for_now()` dispatch through, so changing the scheme from
// the command palette flips the entire app's accent on the next frame.

use egui::Color32;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;

#[derive(Clone, Copy, Debug)]
pub struct Hsl {
    /// Hue in degrees, 0..360.
    pub h: f32,
    /// Saturation, 0..1.
    pub s: f32,
    /// Lightness, 0..1.
    pub l: f32,
}

impl Hsl {
    pub const fn new(h: f32, s: f32, l: f32) -> Self {
        Self { h, s, l }
    }

    pub fn to_color32(self) -> Color32 {
        let (r, g, b) = hsl_to_rgb(self.h, self.s, self.l);
        Color32::from_rgb(r, g, b)
    }
}

/// Shape of the interpolation curve between two anchor hues. Names and
/// behaviour mirror Poline's `positionFunctions`. Only `Sinusoidal` is
/// currently used; the other variants are kept for callers that want to
/// pick a different curve without changing this module.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub enum Curve {
    Linear,
    Sinusoidal,
    Arc,
    Quadratic,
}

impl Curve {
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Curve::Linear => t,
            Curve::Sinusoidal => 0.5 - (t * std::f32::consts::PI).cos() * 0.5,
            Curve::Arc => (1.0 - (1.0 - t).powi(2)).sqrt(),
            Curve::Quadratic => t * t,
        }
    }
}

// ---------- Poline engine ----------

/// Position function family from Poline. Each curve maps `t ∈ [0, 1]`
/// to another `[0, 1]` value; the resulting `t'` drives lerp on a single
/// axis in XYZ space. Different axes can use different functions to
/// bend hue-vs-saturation distribution independently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum PositionFn {
    Linear,
    Sinusoidal,
    ASinusoidal,
    Exponential,
    Quadratic,
    Cubic,
    Quartic,
    Arc,
    SmoothStep,
}

impl PositionFn {
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            PositionFn::Linear => t,
            PositionFn::Sinusoidal => (t * std::f32::consts::FRAC_PI_2).sin(),
            PositionFn::ASinusoidal => t.asin() / std::f32::consts::FRAC_PI_2,
            PositionFn::Exponential => t * t,
            PositionFn::Quadratic => t * t * t,
            PositionFn::Cubic => t.powi(4),
            PositionFn::Quartic => t.powi(5),
            PositionFn::Arc => 1.0 - (1.0 - t).sqrt(),
            PositionFn::SmoothStep => t * t * (3.0 - 2.0 * t),
        }
    }
}

/// Polar XYZ coordinate on the Poline color disk:
/// * `x = 0.5 + l * 0.5 * cos(h_rad)`
/// * `y = 0.5 + l * 0.5 * sin(h_rad)`
/// * `z = s` (saturation along an orthogonal axis)
///
/// Lifting HSL into this representation lets us straight-line lerp
/// between anchors in 3D and recover non-uniform hue progression.
fn hsl_to_xyz(c: Hsl) -> (f32, f32, f32) {
    let h_rad = c.h.to_radians();
    let dist = c.l * 0.5;
    let x = 0.5 + dist * h_rad.cos();
    let y = 0.5 + dist * h_rad.sin();
    let z = c.s;
    (x, y, z)
}

/// Inverse of `hsl_to_xyz`. Hue is the angle of `(x, y)` from the disk
/// center; lightness is the radius scaled to `[0, 1]`; saturation is
/// `z` clamped.
fn xyz_to_hsl(x: f32, y: f32, z: f32) -> Hsl {
    let dx = x - 0.5;
    let dy = y - 0.5;
    let radians = dy.atan2(dx);
    let mut h = radians.to_degrees();
    if h < 0.0 {
        h += 360.0;
    }
    let dist = (dx * dx + dy * dy).sqrt();
    let l = (dist / 0.5).clamp(0.0, 1.0);
    let s = z.clamp(0.0, 1.0);
    Hsl::new(h, s, l)
}

/// A Poline-style palette: a chain of HSL anchors with `points_per_segment`
/// interpolated samples generated between each adjacent pair (in XYZ
/// space, then converted back). When `closed` is true, the final anchor
/// also lerps back to the first, producing a loopable palette.
#[derive(Clone)]
#[allow(dead_code)]
pub struct Poline {
    pub anchors: Vec<Hsl>,
    pub points_per_segment: usize,
    pub fn_x: PositionFn,
    pub fn_y: PositionFn,
    pub fn_z: PositionFn,
    pub closed: bool,
}

#[allow(dead_code)]
impl Poline {
    /// Two-anchor Poline with sane defaults: 4 points per segment,
    /// sinusoidal easing on every axis (matches Poline's TS default).
    pub fn new(anchors: Vec<Hsl>) -> Self {
        Self {
            anchors,
            points_per_segment: 4,
            fn_x: PositionFn::Sinusoidal,
            fn_y: PositionFn::Sinusoidal,
            fn_z: PositionFn::Sinusoidal,
            closed: false,
        }
    }

    /// Generate the full palette as `Color32`s. Boundary samples between
    /// consecutive segments are deduped — each anchor appears exactly
    /// once in the output.
    pub fn colors(&self) -> Vec<Color32> {
        if self.anchors.is_empty() {
            return Vec::new();
        }
        if self.anchors.len() == 1 {
            return vec![self.anchors[0].to_color32()];
        }
        let n = self.points_per_segment.max(2);
        let mut out: Vec<Color32> = Vec::new();
        let pair_count = if self.closed {
            self.anchors.len()
        } else {
            self.anchors.len() - 1
        };
        for seg in 0..pair_count {
            let a = self.anchors[seg];
            let b = self.anchors[(seg + 1) % self.anchors.len()];
            let (ax, ay, az) = hsl_to_xyz(a);
            let (bx, by, bz) = hsl_to_xyz(b);
            // Last segment includes its endpoint; intermediate segments
            // skip it to avoid duplicate anchor samples.
            let skip_last = seg + 1 < pair_count;
            let upper = if skip_last { n - 1 } else { n };
            for i in 0..upper {
                let t = i as f32 / (n as f32 - 1.0);
                let tx = self.fn_x.apply(t);
                let ty = self.fn_y.apply(t);
                let tz = self.fn_z.apply(t);
                let x = lerp(ax, bx, tx);
                let y = lerp(ay, by, ty);
                let z = lerp(az, bz, tz);
                out.push(xyz_to_hsl(x, y, z).to_color32());
            }
        }
        out
    }

    /// Convenience: the palette's "primary" color — the first generated
    /// sample. Used by `accent_now()`.
    pub fn accent(&self) -> Color32 {
        self.colors()
            .first()
            .copied()
            .unwrap_or(Color32::from_rgb(180, 130, 60))
    }

    /// Return the first two anchors as `(primary, secondary)` for
    /// callers that previously consumed `anchors_for_now()`.
    pub fn anchor_pair(&self) -> (Hsl, Hsl) {
        let a = self.anchors.first().copied().unwrap_or(Hsl::new(40.0, 0.7, 0.55));
        let b = self
            .anchors
            .get(1)
            .copied()
            .unwrap_or_else(|| Hsl::new(normalize_hue(a.h + 40.0), a.s * 0.85, a.l * 0.85));
        (a, b)
    }
}

// ---------- Named schemes ----------

/// Schemes the user can switch between via the command palette. Order
/// matches `SCHEMES` and the on-disk atomic representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scheme {
    TimeOfDay,
    Sunset,
    Forest,
    Tide,
    Ember,
    Moss,
    Plum,
    Mono,
    /// User-edited Poline backed by `CUSTOM_POLINE`. Falls back to
    /// `TimeOfDay` if the slot is empty (e.g. on first launch before
    /// the user has touched the editor).
    Custom,
}

impl Scheme {
    pub const ALL: &'static [Scheme] = &[
        Scheme::TimeOfDay,
        Scheme::Sunset,
        Scheme::Forest,
        Scheme::Tide,
        Scheme::Ember,
        Scheme::Moss,
        Scheme::Plum,
        Scheme::Mono,
        Scheme::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Scheme::TimeOfDay => "Time of Day",
            Scheme::Sunset => "Sunset",
            Scheme::Forest => "Forest",
            Scheme::Tide => "Tide",
            Scheme::Ember => "Ember",
            Scheme::Moss => "Moss",
            Scheme::Plum => "Plum",
            Scheme::Mono => "Mono",
            Scheme::Custom => "Custom",
        }
    }

    /// Build the Poline that backs this scheme. `TimeOfDay` is dynamic
    /// — its anchors come from local wall-clock; the rest are static.
    pub fn poline(self) -> Poline {
        match self {
            Scheme::TimeOfDay => time_of_day_poline(),
            Scheme::Sunset => Poline {
                anchors: vec![
                    Hsl::new(20.0, 0.85, 0.62),  // coral peach
                    Hsl::new(330.0, 0.70, 0.55), // magenta
                    Hsl::new(265.0, 0.55, 0.40), // indigo
                ],
                points_per_segment: 5,
                fn_x: PositionFn::Sinusoidal,
                fn_y: PositionFn::Sinusoidal,
                fn_z: PositionFn::Linear,
                closed: false,
            },
            Scheme::Forest => Poline {
                anchors: vec![
                    Hsl::new(95.0, 0.45, 0.45),  // moss
                    Hsl::new(140.0, 0.35, 0.55), // sage
                    Hsl::new(75.0, 0.55, 0.55),  // fern
                ],
                points_per_segment: 5,
                fn_x: PositionFn::Sinusoidal,
                fn_y: PositionFn::Sinusoidal,
                fn_z: PositionFn::Linear,
                closed: false,
            },
            Scheme::Tide => Poline {
                anchors: vec![
                    Hsl::new(195.0, 0.75, 0.30), // deep teal
                    Hsl::new(170.0, 0.55, 0.55), // seafoam
                    Hsl::new(210.0, 0.60, 0.65), // sky
                ],
                points_per_segment: 5,
                fn_x: PositionFn::Sinusoidal,
                fn_y: PositionFn::Sinusoidal,
                fn_z: PositionFn::Linear,
                closed: false,
            },
            Scheme::Ember => Poline {
                anchors: vec![
                    Hsl::new(15.0, 0.80, 0.45),  // burnt orange
                    Hsl::new(0.0, 0.75, 0.50),   // red
                    Hsl::new(320.0, 0.65, 0.55), // magenta
                ],
                points_per_segment: 5,
                fn_x: PositionFn::Sinusoidal,
                fn_y: PositionFn::Sinusoidal,
                fn_z: PositionFn::Linear,
                closed: false,
            },
            Scheme::Moss => Poline {
                anchors: vec![
                    Hsl::new(60.0, 0.50, 0.42), // olive
                    Hsl::new(45.0, 0.60, 0.55), // khaki
                    Hsl::new(40.0, 0.70, 0.55), // mustard
                ],
                points_per_segment: 5,
                fn_x: PositionFn::Sinusoidal,
                fn_y: PositionFn::Sinusoidal,
                fn_z: PositionFn::Linear,
                closed: false,
            },
            Scheme::Plum => Poline {
                anchors: vec![
                    Hsl::new(280.0, 0.45, 0.65), // lavender
                    Hsl::new(310.0, 0.55, 0.45), // plum
                    Hsl::new(345.0, 0.65, 0.35), // wine
                ],
                points_per_segment: 5,
                fn_x: PositionFn::Sinusoidal,
                fn_y: PositionFn::Sinusoidal,
                fn_z: PositionFn::Linear,
                closed: false,
            },
            Scheme::Mono => Poline {
                anchors: vec![
                    Hsl::new(220.0, 0.05, 0.78),
                    Hsl::new(220.0, 0.10, 0.50),
                    Hsl::new(220.0, 0.20, 0.30),
                ],
                points_per_segment: 5,
                fn_x: PositionFn::Linear,
                fn_y: PositionFn::Linear,
                fn_z: PositionFn::Linear,
                closed: false,
            },
            Scheme::Custom => CUSTOM_POLINE
                .lock()
                .ok()
                .and_then(|g| g.clone())
                .unwrap_or_else(time_of_day_poline),
        }
    }

    pub fn from_index(i: u8) -> Self {
        Self::ALL.get(i as usize).copied().unwrap_or(Scheme::TimeOfDay)
    }

    pub fn to_index(self) -> u8 {
        Self::ALL
            .iter()
            .position(|&s| s == self)
            .unwrap_or(0) as u8
    }
}

/// Active scheme index. Lock-free read on every `accent_now()` call.
static ACTIVE_SCHEME: AtomicU8 = AtomicU8::new(0); // Scheme::TimeOfDay

/// User-edited Poline, written by the palette editor. Read every time
/// `Scheme::Custom.poline()` is invoked. `None` until the user touches
/// the editor — in that case `Custom` falls back to `TimeOfDay`.
static CUSTOM_POLINE: Mutex<Option<Poline>> = Mutex::new(None);

/// Snapshot the current custom Poline, if any. Used by the editor to
/// hydrate its initial state from the last user edit.
#[allow(dead_code)]
pub fn custom_poline() -> Option<Poline> {
    CUSTOM_POLINE.lock().ok().and_then(|g| g.clone())
}

/// Replace the custom Poline. Doesn't auto-activate `Scheme::Custom` —
/// callers decide whether to also call `set_active_scheme`.
#[allow(dead_code)]
pub fn set_custom_poline(p: Poline) {
    if let Ok(mut g) = CUSTOM_POLINE.lock() {
        *g = Some(p);
    }
}

#[allow(dead_code)]
pub fn active_scheme() -> Scheme {
    Scheme::from_index(ACTIVE_SCHEME.load(Ordering::Relaxed))
}

#[allow(dead_code)]
pub fn set_active_scheme(s: Scheme) {
    ACTIVE_SCHEME.store(s.to_index(), Ordering::Relaxed);
}

/// Full color palette of the active scheme. Cached per call — callers
/// that draw many items should hoist this out of their loop.
pub fn active_palette() -> Vec<Color32> {
    active_scheme().poline().colors()
}

/// Pick a color from the active palette using a stable hash of `key`.
/// Used to color tag chips, graph nodes, etc. so two same-named items
/// always get the same color, but different names spread across the
/// palette.
pub fn color_for_key(key: &str) -> Color32 {
    let palette = active_palette();
    if palette.is_empty() {
        return accent_now();
    }
    // Tiny FNV-1a 32-bit hash. We don't need cryptographic strength;
    // just stable distribution across the palette indices.
    let mut h: u32 = 0x811C_9DC5;
    for b in key.as_bytes() {
        h ^= u32::from(*b);
        h = h.wrapping_mul(0x0100_0193);
    }
    palette[(h as usize) % palette.len()]
}

/// Build the time-of-day Poline — same logic as the legacy
/// `anchors_for_now()` but expressed as a 3-anchor scheme so it produces
/// gradients comparable to the static presets.
fn time_of_day_poline() -> Poline {
    use chrono::{Datelike, Local, Timelike};
    let now = Local::now();
    let hour = now.hour();
    let month0 = now.month0();
    let base = time_of_day_hue(hour) + season_drift(month0);
    let s = time_saturation(hour);
    Poline {
        anchors: vec![
            Hsl::new(normalize_hue(base), s, 0.58),
            Hsl::new(normalize_hue(base + 40.0), s * 0.85, 0.48),
            Hsl::new(normalize_hue(base + 90.0), s * 0.70, 0.40),
        ],
        points_per_segment: 5,
        fn_x: PositionFn::Sinusoidal,
        fn_y: PositionFn::Sinusoidal,
        fn_z: PositionFn::Linear,
        closed: false,
    }
}

// ---------- Public legacy API (now dispatches through active scheme) ----------

/// Generate `count` colors interpolated between two anchor HSL points.
/// `count == 1` returns the midpoint; `count == 0` returns empty.
pub fn interpolate(a: Hsl, b: Hsl, count: usize, curve: Curve) -> Vec<Color32> {
    if count == 0 {
        return Vec::new();
    }
    if count == 1 {
        return vec![lerp_hsl(a, b, 0.5).to_color32()];
    }
    (0..count)
        .map(|i| {
            let t = i as f32 / (count - 1) as f32;
            let curved = curve.apply(t);
            lerp_hsl(a, b, curved).to_color32()
        })
        .collect()
}

/// The hue the astro-blog's palette would anchor on right now, based on
/// local wall-clock time. Dawn warm, midday cool, dusk amber, night teal.
pub fn time_of_day_hue(hour: u32) -> f32 {
    match hour {
        5..=8 => 20.0,     // dawn — coral/peach
        9..=13 => 200.0,   // midday — bright blue
        14..=17 => 35.0,   // afternoon — amber/gold
        18..=20 => 270.0,  // dusk — purple/violet
        _ => 180.0,        // night — deep teal
    }
}

/// Small seasonal drift on top of the time-of-day hue.
pub fn season_drift(month0: u32) -> f32 {
    match month0 {
        2..=4 => 10.0,    // spring — slight warm push
        5..=7 => 25.0,    // summer — warmer
        8..=10 => -10.0,  // fall — cooler amber
        _ => -20.0,       // winter — cool
    }
}

/// First two anchors of the active scheme, returned in the legacy
/// `(primary, secondary)` shape so existing call sites keep working.
/// `Scheme::TimeOfDay` (default) reproduces the dynamic dawn / midday /
/// dusk / night palette; static schemes return their first two anchors.
pub fn anchors_for_now() -> (Hsl, Hsl) {
    active_scheme().poline().anchor_pair()
}

/// The single "accent color" the rest of the app should use — the
/// first generated sample of the active scheme's Poline. Static
/// schemes return a fixed accent; `TimeOfDay` shifts through the day.
pub fn accent_now() -> Color32 {
    active_scheme().poline().accent()
}

fn time_saturation(hour: u32) -> f32 {
    match hour {
        9..=17 => 0.80,
        6..=8 | 18..=20 => 0.70,
        _ => 0.55,
    }
}

fn normalize_hue(h: f32) -> f32 {
    ((h % 360.0) + 360.0) % 360.0
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Interpolate HSL along the shorter path around the hue circle.
fn lerp_hsl(a: Hsl, b: Hsl, t: f32) -> Hsl {
    let dh = {
        let raw = b.h - a.h;
        // Take the shorter direction around the circle.
        if raw > 180.0 {
            raw - 360.0
        } else if raw < -180.0 {
            raw + 360.0
        } else {
            raw
        }
    };
    Hsl {
        h: normalize_hue(a.h + dh * t),
        s: lerp(a.s, b.s, t),
        l: lerp(a.l, b.l, t),
    }
}

/// HSL → sRGB conversion (standard formula). Inputs: h in degrees, s/l in 0..1.
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let h = normalize_hue(h) / 360.0;
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);

    if s == 0.0 {
        let v = (l * 255.0).round() as u8;
        return (v, v, v);
    }

    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;

    let r = hue_to_rgb(p, q, h + 1.0 / 3.0);
    let g = hue_to_rgb(p, q, h);
    let b = hue_to_rgb(p, q, h - 1.0 / 3.0);
    (
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
    )
}

fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        p + (q - p) * 6.0 * t
    } else if t < 1.0 / 2.0 {
        q
    } else if t < 2.0 / 3.0 {
        p + (q - p) * (2.0 / 3.0 - t) * 6.0
    } else {
        p
    }
}

// ---------- tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_color_eq(a: Color32, b: Color32, tolerance: i32) {
        let ar = a.to_array();
        let br = b.to_array();
        for i in 0..3 {
            assert!(
                (ar[i] as i32 - br[i] as i32).abs() <= tolerance,
                "channel {} differs: {:?} vs {:?}",
                i,
                ar,
                br,
            );
        }
    }

    #[test]
    fn hsl_red_roundtrip() {
        let c = Hsl::new(0.0, 1.0, 0.5).to_color32();
        assert_color_eq(c, Color32::from_rgb(255, 0, 0), 1);
    }

    #[test]
    fn hsl_green_roundtrip() {
        let c = Hsl::new(120.0, 1.0, 0.5).to_color32();
        assert_color_eq(c, Color32::from_rgb(0, 255, 0), 1);
    }

    #[test]
    fn hsl_grayscale_zero_saturation() {
        let c = Hsl::new(200.0, 0.0, 0.5).to_color32();
        assert_color_eq(c, Color32::from_rgb(128, 128, 128), 1);
    }

    #[test]
    fn interpolate_produces_n_colors() {
        let a = Hsl::new(0.0, 1.0, 0.5);
        let b = Hsl::new(120.0, 1.0, 0.5);
        assert_eq!(interpolate(a, b, 5, Curve::Linear).len(), 5);
        assert_eq!(interpolate(a, b, 1, Curve::Linear).len(), 1);
        assert_eq!(interpolate(a, b, 0, Curve::Linear).len(), 0);
    }

    #[test]
    fn interpolate_endpoints_are_anchors() {
        let a = Hsl::new(0.0, 1.0, 0.5);
        let b = Hsl::new(120.0, 1.0, 0.5);
        let p = interpolate(a, b, 3, Curve::Linear);
        assert_color_eq(p[0], a.to_color32(), 1);
        assert_color_eq(p[2], b.to_color32(), 1);
    }

    #[test]
    fn hue_interpolation_takes_short_path() {
        // 350° → 10° should go through 0°, not sweep across 180°.
        let a = Hsl::new(350.0, 1.0, 0.5);
        let b = Hsl::new(10.0, 1.0, 0.5);
        let mid = lerp_hsl(a, b, 0.5);
        // Midpoint should be near 0° / 360°, not 180°.
        assert!(mid.h < 20.0 || mid.h > 340.0, "expected ≈0°, got {}", mid.h);
    }

    #[test]
    fn curves_are_monotonic_and_bounded() {
        for curve in [Curve::Linear, Curve::Sinusoidal, Curve::Arc, Curve::Quadratic] {
            assert_eq!(curve.apply(0.0), 0.0);
            let v1 = curve.apply(1.0);
            assert!((v1 - 1.0).abs() < 1e-5, "curve {:?} end = {}", curve, v1);
            // Monotonic increasing
            let mut last = -1.0;
            for i in 0..=10 {
                let t = i as f32 / 10.0;
                let v = curve.apply(t);
                assert!(v >= last - 1e-5, "{:?} not monotonic at {}", curve, t);
                last = v;
            }
        }
    }

    #[test]
    fn time_of_day_bias_is_stable_over_ranges() {
        // Same band returns same hue.
        assert_eq!(time_of_day_hue(10), time_of_day_hue(12));
        assert_eq!(time_of_day_hue(2), time_of_day_hue(4));
        // Different bands differ.
        assert_ne!(time_of_day_hue(7), time_of_day_hue(15));
    }

    #[test]
    fn xyz_roundtrip_preserves_anchor() {
        // Lifting a saturated mid-lightness color into XYZ and back
        // should preserve hue and saturation; lightness has minor float
        // drift but stays close.
        let a = Hsl::new(120.0, 0.7, 0.55);
        let (x, y, z) = hsl_to_xyz(a);
        let b = xyz_to_hsl(x, y, z);
        assert!((a.h - b.h).abs() < 0.5, "hue: {} vs {}", a.h, b.h);
        assert!((a.s - b.s).abs() < 1e-3, "sat: {} vs {}", a.s, b.s);
        assert!((a.l - b.l).abs() < 1e-3, "lightness: {} vs {}", a.l, b.l);
    }

    #[test]
    fn poline_starts_and_ends_at_anchors() {
        let p = Poline::new(vec![
            Hsl::new(20.0, 0.8, 0.6),
            Hsl::new(200.0, 0.5, 0.5),
        ]);
        let cs = p.colors();
        assert!(cs.len() >= 2);
        // Endpoints should match the input anchors (within rounding).
        assert_color_eq(cs[0], p.anchors[0].to_color32(), 2);
        assert_color_eq(*cs.last().unwrap(), p.anchors[1].to_color32(), 2);
    }

    #[test]
    fn poline_emits_correct_count_for_chain() {
        // 3 anchors, 5 points/seg, open: 5 + 4 = 9 colors (boundary deduped)
        let p = Poline {
            anchors: vec![
                Hsl::new(0.0, 0.7, 0.5),
                Hsl::new(120.0, 0.7, 0.5),
                Hsl::new(240.0, 0.7, 0.5),
            ],
            points_per_segment: 5,
            fn_x: PositionFn::Linear,
            fn_y: PositionFn::Linear,
            fn_z: PositionFn::Linear,
            closed: false,
        };
        assert_eq!(p.colors().len(), 9);
    }

    #[test]
    fn position_fn_endpoints_are_zero_and_one() {
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
            assert!(f.apply(0.0).abs() < 1e-4, "{:?}(0) != 0", f);
            assert!((f.apply(1.0) - 1.0).abs() < 1e-4, "{:?}(1) != 1", f);
        }
    }

    #[test]
    fn scheme_round_trips_through_atomic_index() {
        for s in Scheme::ALL {
            assert_eq!(Scheme::from_index(s.to_index()), *s);
        }
    }

    #[test]
    fn active_scheme_switches() {
        set_active_scheme(Scheme::Sunset);
        assert_eq!(active_scheme(), Scheme::Sunset);
        set_active_scheme(Scheme::TimeOfDay); // restore default for other tests
        assert_eq!(active_scheme(), Scheme::TimeOfDay);
    }
}
