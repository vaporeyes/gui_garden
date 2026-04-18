use egui::{Color32, RichText, Ui};

use crate::palette;

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct AboutMe {}

impl Default for AboutMe {
    fn default() -> Self {
        Self {}
    }
}

/// A skill and a qualitative strength, 0..1. The rendered chip's background
/// saturation and text weight scale with the value, but no numeric percentage
/// is surfaced — these are impressions, not benchmarks.
struct Skill {
    name: &'static str,
    level: f32,
}

struct SkillGroup {
    label: &'static str,
    skills: &'static [Skill],
}

const SKILL_GROUPS: &[SkillGroup] = &[
    SkillGroup {
        label: "languages",
        skills: &[
            Skill { name: "Python",     level: 0.90 },
            Skill { name: "Go",         level: 0.80 },
            Skill { name: "TypeScript", level: 0.75 },
            Skill { name: "JavaScript", level: 0.65 },
            Skill { name: "Swift",      level: 0.55 },
            Skill { name: "Elixir",     level: 0.45 },
            Skill { name: "Rust",       level: 0.40 },
            Skill { name: "Shell",      level: 0.80 },
        ],
    },
    SkillGroup {
        label: "infrastructure",
        skills: &[
            Skill { name: "AWS",        level: 0.85 },
            Skill { name: "Docker",     level: 0.85 },
            Skill { name: "Kubernetes", level: 0.80 },
            Skill { name: "Terraform",  level: 0.80 },
            Skill { name: "Helm",       level: 0.70 },
            Skill { name: "Cloudflare", level: 0.75 },
            Skill { name: "Lambda",     level: 0.80 },
        ],
    },
    SkillGroup {
        label: "data & storage",
        skills: &[
            Skill { name: "PostgreSQL", level: 0.75 },
            Skill { name: "DynamoDB",   level: 0.80 },
            Skill { name: "SQLite",     level: 0.65 },
            Skill { name: "Redis",      level: 0.55 },
        ],
    },
    SkillGroup {
        label: "frontend",
        skills: &[
            Skill { name: "Astro",      level: 0.75 },
            Skill { name: "SvelteKit",  level: 0.65 },
            Skill { name: "React",      level: 0.60 },
            Skill { name: "Phaser",     level: 0.50 },
            Skill { name: "SwiftUI",    level: 0.50 },
        ],
    },
];

struct Focus {
    name: &'static str,
    blurb: &'static str,
    url: Option<&'static str>,
}

const CURRENTLY_BUILDING: &[Focus] = &[
    Focus {
        name: "LiftLog",
        blurb: "Full-stack weightlifting tracker with Strong CSV import, strength standards, and AI insights",
        url: None,
    },
    Focus {
        name: "Elegy Campaign Player",
        blurb: "Solo vampire TTRPG webapp — 19 TypeScript engine modules, 886 tests, optional LLM narration",
        url: None,
    },
    Focus {
        name: "josh.bot",
        blurb: "Go API on Lambda + DynamoDB powering the dynamic data across my sites",
        url: Some("https://github.com/vaporeyes/josh.bot"),
    },
    Focus {
        name: "gui garden",
        blurb: "This thing — a Rust/egui digital garden for local Markdown notes",
        url: None,
    },
];

struct Interest {
    icon: &'static str,
    label: &'static str,
}

const INTERESTS: &[Interest] = &[
    Interest { icon: "🏋",  label: "lifting" },
    Interest { icon: "📚", label: "reading" },
    Interest { icon: "🎲", label: "tabletop" },
    Interest { icon: "🎧", label: "music production" },
    Interest { icon: "🍳", label: "cooking" },
    Interest { icon: "🪵", label: "crafts" },
];

impl AboutMe {
    pub fn ui(&mut self, ui: &mut Ui) {
        let accent = palette::accent_now();
        let muted = ui.visuals().weak_text_color();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_max_width(560.0);

                // ---- Header ----
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Josh")
                        .size(34.0)
                        .strong()
                        .color(accent),
                );
                ui.label(
                    RichText::new("SRE / Backend / Platform engineer — Middle Tennessee")
                        .size(14.0)
                        .italics()
                        .color(muted),
                );
                ui.add_space(8.0);
                ui.label(
                    "Building infrastructure, weird apps, lifting heavy things, \
                     and making stuff. Usually somewhere between the terminal \
                     and the rack.",
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.hyperlink_to(
                        RichText::new("github").color(accent),
                        "https://github.com/vaporeyes",
                    );
                    ui.label(RichText::new("·").weak());
                    ui.hyperlink_to(
                        RichText::new("josh.contact").color(accent),
                        "https://josh.contact",
                    );
                });

                ui.add_space(18.0);
                ui.separator();

                // ---- Skills ----
                ui.add_space(10.0);
                small_caps(ui, "skills", accent);
                ui.add_space(6.0);
                for group in SKILL_GROUPS {
                    render_skill_group(ui, group, accent);
                    ui.add_space(8.0);
                }

                ui.add_space(8.0);
                ui.separator();

                // ---- Currently building ----
                ui.add_space(10.0);
                small_caps(ui, "currently building", accent);
                ui.add_space(4.0);
                for focus in CURRENTLY_BUILDING {
                    render_focus(ui, focus, accent, muted);
                    ui.add_space(4.0);
                }

                ui.add_space(8.0);
                ui.separator();

                // ---- Interests ----
                ui.add_space(10.0);
                small_caps(ui, "also", accent);
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;
                    for interest in INTERESTS {
                        ui.label(
                            RichText::new(format!("{}  {}", interest.icon, interest.label))
                                .size(13.0),
                        );
                    }
                });

                ui.add_space(20.0);
            });
    }
}

// ---------- rendering helpers ----------

fn small_caps(ui: &mut Ui, text: &str, color: Color32) {
    ui.label(
        RichText::new(text.to_uppercase())
            .small()
            .strong()
            .color(color),
    );
}

fn render_skill_group(ui: &mut Ui, group: &SkillGroup, accent: Color32) {
    ui.label(
        RichText::new(group.label)
            .size(12.0)
            .color(ui.visuals().weak_text_color()),
    );
    ui.add_space(2.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
        for skill in group.skills {
            render_skill_chip(ui, skill, accent);
        }
    });
}

fn render_skill_chip(ui: &mut Ui, skill: &Skill, accent: Color32) {
    // Chip background brightens with proficiency; text stays legible at all
    // levels. Size stays fixed — no eye-catching scale tricks that would
    // make weaker skills look hidden.
    let bg_alpha = 0.10 + 0.35 * skill.level.clamp(0.0, 1.0);
    let bg = accent.linear_multiply(bg_alpha);

    let mut text = RichText::new(skill.name).size(13.0).color(accent);
    if skill.level >= 0.75 {
        text = text.strong();
    }

    egui::Frame::NONE
        .fill(bg)
        .inner_margin(egui::Margin::symmetric(9, 4))
        .corner_radius(egui::CornerRadius::same(10))
        .show(ui, |ui| {
            ui.label(text);
        });
}

fn render_focus(ui: &mut Ui, focus: &Focus, accent: Color32, muted: Color32) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let name = RichText::new(focus.name).strong().color(accent);
        match focus.url {
            Some(url) => {
                ui.hyperlink_to(name, url);
            }
            None => {
                ui.label(name);
            }
        }
        ui.label(RichText::new("—").color(muted));
        ui.label(RichText::new(focus.blurb).color(muted));
    });
}
