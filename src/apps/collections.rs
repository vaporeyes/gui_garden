// Collections viewer — mirrors the data model from OmniCollect.
//
// Renders a gallery of items (sample data: Roman Imperial coins) with
// obverse + reverse imagery, provenance chain, and grading. The static
// `SAMPLE_ITEMS` below is a placeholder; point `load_from_path` at a
// real export (JSON matching the `CollectionFile` schema) or wire up a
// `☁ Fetch from josh.bot` button analogous to the one in `Workouts` /
// `Projects` once the API endpoint exists.

use egui::{Color32, Pos2, Rect, RichText, Ui, Vec2};
use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::palette;

#[derive(Debug, Deserialize, Clone)]
pub struct CollectionFile {
    pub title: Option<String>,
    #[serde(default)]
    pub items: Vec<CollectionItem>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CollectionItem {
    pub id: String,
    pub name: String,
    /// Short attribution line, e.g. "Augustus · AR Denarius · 27 BC – AD 14".
    pub attribution: Option<String>,
    /// Mint / region if known.
    pub mint: Option<String>,
    /// Grading (VG/F/VF/XF/AU/MS, or numeric).
    pub grade: Option<String>,
    /// Weight in grams, when recorded. Parsed but not yet surfaced in the
    /// UI — reserved for a future numismatic-specs row in the detail panel.
    #[allow(dead_code)]
    pub weight_g: Option<f32>,
    /// Diameter in millimetres.
    #[allow(dead_code)]
    pub diameter_mm: Option<f32>,
    /// URLs to obverse / reverse images. `file://` works for local paths.
    pub obverse: Option<String>,
    pub reverse: Option<String>,
    /// Ordered provenance list, oldest first.
    #[serde(default)]
    pub provenance: Vec<String>,
    /// Free-form notes — rendered below the attribution block.
    pub notes: Option<String>,
}

/// Placeholder data so the gallery renders something on first launch.
/// Replace with a real export once OmniCollect lands a JSON endpoint.
const SAMPLE_ITEMS: &[SampleItem] = &[
    SampleItem {
        id: "augustus-denarius",
        name: "Augustus — Denarius",
        attribution: "Augustus · AR Denarius · 27 BC – AD 14 · 3.85g",
        mint: "Lugdunum",
        grade: "EF",
        provenance: &[
            "Glendining's, London, 1965",
            "Private collection, NY, 1982",
            "Heritage Auctions, 2019",
        ],
        notes: "Laureate head right; Gaius and Lucius standing facing, holding shields and spears.",
    },
    SampleItem {
        id: "trajan-sestertius",
        name: "Trajan — Sestertius",
        attribution: "Trajan · AE Sestertius · AD 98 – 117 · 26.1g",
        mint: "Rome",
        grade: "VF+",
        provenance: &[
            "Gemini, LLC, 2011",
            "Private European collection",
        ],
        notes: "Dacia seated left on pile of arms, mourning. A visually striking late-issue bronze.",
    },
    SampleItem {
        id: "marcus-aurelius-denarius",
        name: "Marcus Aurelius — Denarius",
        attribution: "Marcus Aurelius · AR Denarius · AD 161 – 180 · 3.32g",
        mint: "Rome",
        grade: "XF",
        provenance: &[
            "CNG 405, 2017",
            "Private US collection",
        ],
        notes: "Equity standing left holding scales. Philosopher-emperor; late-reign portrait.",
    },
    SampleItem {
        id: "constantine-solidus",
        name: "Constantine I — Solidus",
        attribution: "Constantine the Great · AV Solidus · AD 306 – 337 · 4.48g",
        mint: "Nicomedia",
        grade: "AU",
        provenance: &[
            "NAC 110, 2018",
            "Ex. Important Roman Gold Collection",
        ],
        notes: "Transitional issue — the solidus would go on to underpin the Byzantine monetary system.",
    },
];

struct SampleItem {
    id: &'static str,
    name: &'static str,
    attribution: &'static str,
    mint: &'static str,
    grade: &'static str,
    provenance: &'static [&'static str],
    notes: &'static str,
}

pub struct Collections {
    loaded: Option<(PathBuf, CollectionFile)>,
    /// Selected item id, if any — clicking a card pins it and reveals
    /// the full provenance panel.
    selected: Option<String>,
    error: Option<String>,
    query: String,
}

impl Default for Collections {
    fn default() -> Self {
        Self {
            loaded: None,
            selected: None,
            error: None,
            query: String::new(),
        }
    }
}

impl Collections {
    #[cfg(not(target_arch = "wasm32"))]
    fn pick_and_load(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Collection JSON", &["json"])
            .set_title("Load an OmniCollect export")
            .pick_file()
        else {
            return;
        };
        self.load_from_path(path);
    }

    #[cfg(target_arch = "wasm32")]
    fn pick_and_load(&mut self) {
        self.error = Some("File picker unavailable on web.".into());
    }

    #[allow(dead_code)] // invoked natively via pick_and_load; unused on wasm
    pub fn load_from_path<P: Into<PathBuf>>(&mut self, path: P) {
        let path = path.into();
        match std::fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str::<CollectionFile>(&content) {
                Ok(file) => {
                    self.loaded = Some((path, file));
                    self.error = None;
                }
                Err(e) => self.error = Some(format!("parse error: {}", e)),
            },
            Err(e) => self.error = Some(format!("read error: {}", e)),
        }
    }

    /// Path of the currently-loaded collection, if any. Exposed for the
    /// outer app to persist the last-used path the same way workouts
    /// and canvas files are handled.
    #[allow(dead_code)]
    pub fn loaded_path(&self) -> Option<&Path> {
        self.loaded.as_ref().map(|(p, _)| p.as_path())
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let accent = palette::accent_now();
        let muted = ui.visuals().weak_text_color();

        ui.horizontal(|ui| {
            if ui.button("📁 Load collection…").clicked() {
                self.pick_and_load();
            }
            if let Some((_, file)) = &self.loaded {
                ui.label(
                    RichText::new(format!(
                        "{}  ({} items)",
                        file.title.clone().unwrap_or_else(|| "Collection".into()),
                        file.items.len()
                    ))
                    .weak(),
                );
            } else {
                ui.label(
                    RichText::new("Showing sample Roman coins until a collection is loaded.")
                        .weak(),
                );
            }
        });
        if let Some(err) = &self.error {
            ui.colored_label(Color32::from_rgb(220, 80, 80), err);
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("🔍").weak());
            ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .desired_width(f32::INFINITY)
                    .hint_text("filter by name / attribution / mint"),
            );
        });
        ui.separator();

        // Split: gallery on the left, selected item detail on the right.
        let q = self.query.to_lowercase();
        let matches: Vec<ItemRef<'_>> = match self.loaded.as_ref() {
            Some((_, file)) => file
                .items
                .iter()
                .filter(|it| filter_matches(&q, &it.name, it.attribution.as_deref()))
                .map(ItemRef::Loaded)
                .collect(),
            None => SAMPLE_ITEMS
                .iter()
                .filter(|it| filter_matches(&q, it.name, Some(it.attribution)))
                .map(ItemRef::Sample)
                .collect(),
        };

        let mut clicked_id: Option<String> = None;

        egui::ScrollArea::vertical()
            .id_salt("collections_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let card_size = Vec2::new(220.0, 220.0);
                let cols = ((ui.available_width() / (card_size.x + 8.0)).floor() as usize)
                    .max(1);

                let mut i = 0;
                while i < matches.len() {
                    ui.horizontal(|ui| {
                        for item in matches.iter().skip(i).take(cols) {
                            if render_card(
                                ui,
                                item,
                                card_size,
                                accent,
                                muted,
                                self.selected.as_deref() == Some(item.id()),
                            ) {
                                clicked_id = Some(item.id().to_string());
                            }
                        }
                    });
                    ui.add_space(8.0);
                    i += cols;
                }

                if let Some(sel) = self.selected.as_deref() {
                    if let Some(item) = matches.iter().find(|it| it.id() == sel) {
                        ui.add_space(10.0);
                        ui.separator();
                        ui.add_space(6.0);
                        render_detail(ui, item, accent, muted);
                    }
                }
            });

        if let Some(id) = clicked_id {
            // Toggle: clicking the selected card a second time deselects it.
            if self.selected.as_deref() == Some(id.as_str()) {
                self.selected = None;
            } else {
                self.selected = Some(id);
            }
        }
    }
}

fn filter_matches(q: &str, name: &str, attribution: Option<&str>) -> bool {
    if q.is_empty() {
        return true;
    }
    name.to_lowercase().contains(q)
        || attribution
            .map(|a| a.to_lowercase().contains(q))
            .unwrap_or(false)
}

enum ItemRef<'a> {
    Sample(&'a SampleItem),
    Loaded(&'a CollectionItem),
}

impl<'a> ItemRef<'a> {
    fn id(&self) -> &str {
        match self {
            ItemRef::Sample(s) => s.id,
            ItemRef::Loaded(i) => i.id.as_str(),
        }
    }
    fn name(&self) -> &str {
        match self {
            ItemRef::Sample(s) => s.name,
            ItemRef::Loaded(i) => i.name.as_str(),
        }
    }
    fn attribution(&self) -> Option<&str> {
        match self {
            ItemRef::Sample(s) => Some(s.attribution),
            ItemRef::Loaded(i) => i.attribution.as_deref(),
        }
    }
    fn mint(&self) -> Option<&str> {
        match self {
            ItemRef::Sample(s) => Some(s.mint),
            ItemRef::Loaded(i) => i.mint.as_deref(),
        }
    }
    fn grade(&self) -> Option<&str> {
        match self {
            ItemRef::Sample(s) => Some(s.grade),
            ItemRef::Loaded(i) => i.grade.as_deref(),
        }
    }
    fn obverse(&self) -> Option<&str> {
        match self {
            ItemRef::Sample(_) => None,
            ItemRef::Loaded(i) => i.obverse.as_deref(),
        }
    }
    fn reverse(&self) -> Option<&str> {
        match self {
            ItemRef::Sample(_) => None,
            ItemRef::Loaded(i) => i.reverse.as_deref(),
        }
    }
    fn provenance(&self) -> Vec<String> {
        match self {
            ItemRef::Sample(s) => s.provenance.iter().map(|s| s.to_string()).collect(),
            ItemRef::Loaded(i) => i.provenance.clone(),
        }
    }
    fn notes(&self) -> Option<&str> {
        match self {
            ItemRef::Sample(s) => Some(s.notes),
            ItemRef::Loaded(i) => i.notes.as_deref(),
        }
    }
}

fn render_card(
    ui: &mut Ui,
    item: &ItemRef<'_>,
    size: Vec2,
    accent: Color32,
    muted: Color32,
    is_selected: bool,
) -> bool {
    let response = egui::Frame::NONE
        .fill(ui.visuals().faint_bg_color)
        .inner_margin(egui::Margin::same(8))
        .corner_radius(egui::CornerRadius::same(6))
        .stroke(if is_selected {
            egui::Stroke::new(1.5, accent)
        } else {
            egui::Stroke::NONE
        })
        .show(ui, |ui| {
            ui.set_min_size(size);
            ui.set_max_size(size);
            ui.vertical(|ui| {
                let image_h = 120.0;
                match item.obverse() {
                    Some(src) => {
                        ui.add(
                            egui::Image::new(src)
                                .max_height(image_h)
                                .corner_radius(4.0),
                        );
                    }
                    None => {
                        // Placeholder disc when the item has no image —
                        // renders a coin-silhouette so the card still reads.
                        coin_placeholder(ui, image_h, accent);
                    }
                }
                ui.add_space(6.0);
                ui.label(RichText::new(item.name()).strong().color(accent));
                if let Some(attr) = item.attribution() {
                    ui.label(
                        RichText::new(attr)
                            .small()
                            .color(muted),
                    );
                }
                ui.horizontal(|ui| {
                    if let Some(grade) = item.grade() {
                        ui.label(
                            RichText::new(format!("grade {}", grade))
                                .small()
                                .color(accent),
                        );
                    }
                    if let Some(mint) = item.mint() {
                        ui.label(
                            RichText::new(format!("· {}", mint)).small().weak(),
                        );
                    }
                });
            });
        })
        .response;
    let click_resp = response.interact(egui::Sense::click());
    click_resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

fn coin_placeholder(ui: &mut Ui, h: f32, accent: Color32) {
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(h, h), egui::Sense::hover());
    let c = rect.center();
    let r = (rect.width() / 2.0) - 4.0;
    let painter = ui.painter();
    painter.circle_filled(c, r, accent.linear_multiply(0.15));
    painter.circle_stroke(c, r, egui::Stroke::new(1.5, accent.linear_multiply(0.5)));
    // Faint inner ring for texture.
    painter.circle_stroke(
        c,
        r * 0.78,
        egui::Stroke::new(0.5, accent.linear_multiply(0.35)),
    );
    let _ = Pos2::new(c.x, c.y);
    let _ = Rect::from_center_size(c, Vec2::splat(r * 2.0));
}

fn render_detail(ui: &mut Ui, item: &ItemRef<'_>, accent: Color32, muted: Color32) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(item.name())
                .size(20.0)
                .strong()
                .color(accent),
        );
    });
    if let Some(attr) = item.attribution() {
        ui.label(RichText::new(attr).color(muted));
    }
    ui.add_space(6.0);

    // Both faces side-by-side if we have them.
    ui.horizontal(|ui| {
        if let Some(ob) = item.obverse() {
            ui.vertical(|ui| {
                ui.label(RichText::new("obverse").small().weak());
                ui.add(egui::Image::new(ob).max_height(200.0).corner_radius(4.0));
            });
        }
        if let Some(re) = item.reverse() {
            ui.vertical(|ui| {
                ui.label(RichText::new("reverse").small().weak());
                ui.add(egui::Image::new(re).max_height(200.0).corner_radius(4.0));
            });
        }
    });

    ui.add_space(8.0);
    if let Some(notes) = item.notes() {
        ui.label(RichText::new(notes));
        ui.add_space(6.0);
    }

    let prov = item.provenance();
    if !prov.is_empty() {
        ui.label(RichText::new("PROVENANCE").small().strong().color(accent));
        ui.add_space(2.0);
        for (i, line) in prov.iter().enumerate() {
            ui.label(
                RichText::new(format!("  {}. {}", i + 1, line))
                    .small()
                    .color(muted),
            );
        }
    }
}
