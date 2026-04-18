// Projects catalog, ported from the astro-blog's `src/data/projects.ts`.
// Single source of truth for the projects window — edit the slice below
// to add or rearrange entries.

use egui::{Color32, RichText, Ui};

use crate::palette;

struct Project {
    name: &'static str,
    url: Option<&'static str>,
    description: &'static str,
    stack: &'static [&'static str],
}

const PROJECTS: &[Project] = &[
    Project {
        name: "josh.bot",
        url: Some("https://github.com/vaporeyes/josh.bot/"),
        description:
            "Go API backed by DynamoDB, deployed to Lambda. Powers the dynamic data across most of my sites.",
        stack: &["Go", "DynamoDB", "Lambda"],
    },
    Project {
        name: "k8-one.josh.bot",
        url: Some("https://k8-one.josh.bot"),
        description: "My agent k8-one's personal blog",
        stack: &["Astro", "Cloudflare Pages"],
    },
    Project {
        name: "LiftLog",
        url: None,
        description:
            "Full-stack weightlifting tracker. Strong CSV import, exercise library, strength standards, AI insights.",
        stack: &["Go", "SvelteKit", "SwiftUI", "PostgreSQL"],
    },
    Project {
        name: "Elegy Campaign Player",
        url: None,
        description:
            "Solo vampire TTRPG webapp. 19 TypeScript engine modules, 886 tests, optional LLM narration.",
        stack: &["TypeScript", "React", "OpenRouter"],
    },
    Project {
        name: "bookalysis",
        url: Some("https://github.com/vaporeyes/bookalysis"),
        description:
            "EPUB analysis pipeline with LLM-powered annotations. Three-column web reader.",
        stack: &["Python", "Flask", "AI"],
    },
    Project {
        name: "cartograph",
        url: Some("https://github.com/vaporeyes/cartograph"),
        description:
            "Code mapping tool with style consistency analysis and diff-aware code review.",
        stack: &["Python", "AI"],
    },
    Project {
        name: "movielog",
        url: None,
        description:
            "Media catalog for movies, books, comics, and magazines. TMDB integration for automated metadata.",
        stack: &["Python", "SQLAlchemy", "PostgreSQL"],
    },
    Project {
        name: "autonotes",
        url: Some("https://github.com/vaporeyes/autonotes"),
        description:
            "FastAPI application for Obsidian vault analysis. Auto-triage, note clustering, LLM integration.",
        stack: &["Python", "FastAPI"],
    },
    Project {
        name: "cal",
        url: Some("https://github.com/vaporeyes/calv2"),
        description: "Single-page calendar with data pulling from DynamoDB",
        stack: &["Javascript", "CSS", "Cloudflare Pages"],
    },
    Project {
        name: "alien cannon timeline",
        url: Some("https://alien-timeline.josh.bot/"),
        description:
            "A fun timeline trying to keep track of the increasingly disparate lore of the Alien franchise.",
        stack: &["Javascript", "CSS", "Cloudflare Pages"],
    },
    Project {
        name: "ping sweeper",
        url: Some("https://github.com/vaporeyes/ping-sweep"),
        description:
            "Rust CLI tool for fast ICMP ping sweeps with concurrency and reporting features.",
        stack: &["Rust"],
    },
    Project {
        name: "pb-viewer",
        url: Some("https://github.com/vaporeyes/pb-viewer"),
        description:
            "Mostly golang and typescript code for parsing photos. Spec-kit experimentation.",
        stack: &["Go", "Javascript", "Speckit"],
    },
    Project {
        name: "obsidian-uuidstamper",
        url: Some("https://github.com/vaporeyes/obsidian-uuidstamper"),
        description: "An obsidian uuid timestamper",
        stack: &["TypeScript", "Obsidian API"],
    },
    Project {
        name: "wordle-clone",
        url: Some("https://github.com/vaporeyes/wordle-clone"),
        description: "A wordle clone. yeah.",
        stack: &["Python"],
    },
    Project {
        name: "breakerz",
        url: Some("https://github.com/vaporeyes/breakerz"),
        description: "Brick Breaker game written in Phaser!",
        stack: &["Javascript", "Phaser"],
    },
    Project {
        name: "Metrognomic",
        url: Some("https://github.com/vaporeyes/Metrognomic"),
        description: "Dance with a metrognome. One of my first Swift projects.",
        stack: &["Swift"],
    },
    Project {
        name: "stoicisms",
        url: Some("https://github.com/vaporeyes/stoicisms"),
        description: "Daily stoic quotes in app or a widget, runs on the mac desktop.",
        stack: &["Swift"],
    },
    Project {
        name: "routinerampage",
        url: Some("https://github.com/vaporeyes/routinerampage"),
        description: "A habit tracker modeled with a 90s theme.",
        stack: &["TypeScript"],
    },
    Project {
        name: "k8s-platform",
        url: Some("https://github.com/vaporeyes/k8s-platform"),
        description:
            "A platform for my apps, handles all kinds of various apps and services. Mostly a WIP still but likely always will be.",
        stack: &["Shell", "Kubernetes", "Helm", "Terraform"],
    },
    Project {
        name: "media-stack",
        url: Some("https://github.com/vaporeyes/media-stack"),
        description: "Stackin media, pretty much an entire media system in docker.",
        stack: &["Shell", "Docker"],
    },
];

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct Projects {
    #[cfg_attr(feature = "serde", serde(skip))]
    stack_filter: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip))]
    query: String,
}

impl Default for Projects {
    fn default() -> Self {
        Self {
            stack_filter: None,
            query: String::new(),
        }
    }
}

impl Projects {
    pub fn ui(&mut self, ui: &mut Ui) {
        let accent = palette::accent_now();
        let muted = ui.visuals().weak_text_color();

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("🔍").weak());
            ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .desired_width(f32::INFINITY)
                    .hint_text("filter projects by name or description"),
            );
        });
        if let Some(stack) = self.stack_filter.clone() {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("stack: {}", stack))
                        .small()
                        .color(accent),
                );
                if ui.small_button("clear").clicked() {
                    self.stack_filter = None;
                }
            });
        }
        ui.add_space(8.0);
        ui.separator();

        let q = self.query.to_lowercase();
        let matches: Vec<&Project> = PROJECTS
            .iter()
            .filter(|p| {
                let text_match = q.is_empty()
                    || p.name.to_lowercase().contains(&q)
                    || p.description.to_lowercase().contains(&q);
                let stack_match = self
                    .stack_filter
                    .as_deref()
                    .map(|f| p.stack.iter().any(|s| s == &f))
                    .unwrap_or(true);
                text_match && stack_match
            })
            .collect();

        ui.add_space(6.0);
        ui.label(
            RichText::new(format!(
                "{} project{}",
                matches.len(),
                if matches.len() == 1 { "" } else { "s" }
            ))
            .small()
            .weak(),
        );
        ui.add_space(4.0);

        // Let inner widgets see the click-decision synchronously, but act
        // on stack-filter changes outside the iteration borrow.
        let mut new_filter: Option<String> = None;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for project in &matches {
                    render_project(ui, project, accent, muted, &mut new_filter);
                    ui.add_space(8.0);
                }
            });

        if let Some(f) = new_filter {
            self.stack_filter = Some(f);
        }
    }
}

fn render_project(
    ui: &mut Ui,
    project: &Project,
    accent: Color32,
    muted: Color32,
    new_filter: &mut Option<String>,
) {
    egui::Frame::NONE
        .fill(ui.visuals().faint_bg_color)
        .inner_margin(egui::Margin::same(10))
        .corner_radius(egui::CornerRadius::same(6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let name = RichText::new(project.name).size(16.0).strong().color(accent);
                match project.url {
                    Some(url) => {
                        ui.hyperlink_to(name, url);
                    }
                    None => {
                        ui.label(name);
                    }
                }
            });
            ui.add_space(2.0);
            ui.label(RichText::new(project.description).color(muted));
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                for tag in project.stack {
                    let chip = RichText::new(*tag)
                        .small()
                        .color(accent)
                        .background_color(accent.linear_multiply(0.15));
                    let resp = ui
                        .add(egui::Label::new(chip).sense(egui::Sense::click()))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text(format!("Filter by {}", tag));
                    if resp.clicked() {
                        *new_filter = Some((*tag).to_string());
                    }
                }
            });
        });
}
