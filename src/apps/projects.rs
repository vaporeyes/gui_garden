// Projects catalog, ported from the astro-blog's `src/data/projects.ts`.
// The const slice below is the offline fallback / seed data; the
// "Fetch from josh.bot" button replaces it with the live list from the
// deployed API so the garden stays in sync with the site.

use egui::{Color32, RichText, Ui};
use serde::Deserialize;
use std::sync::{Arc, Mutex};

use crate::palette;

/// Remote projects endpoint. Expected to return a JSON array of
/// `{name, url, description, stack}` records matching the fallback schema.
const PROJECTS_REMOTE_URL: &str = "https://josh.bot/api/projects";

#[derive(Debug, Deserialize, Clone)]
struct RemoteProject {
    name: String,
    url: Option<String>,
    description: String,
    #[serde(default)]
    stack: Vec<String>,
}

/// Display record — static entries come from the const slice, remote
/// entries come from the josh.bot API. The two differ in string
/// ownership, which we bridge with an enum at render time.
enum ProjectRef<'a> {
    Static(&'a Project),
    Remote(&'a RemoteProject),
}

impl<'a> ProjectRef<'a> {
    fn name(&self) -> &str {
        match self {
            ProjectRef::Static(p) => p.name,
            ProjectRef::Remote(p) => p.name.as_str(),
        }
    }
    fn url(&self) -> Option<&str> {
        match self {
            ProjectRef::Static(p) => p.url,
            ProjectRef::Remote(p) => p.url.as_deref(),
        }
    }
    fn description(&self) -> &str {
        match self {
            ProjectRef::Static(p) => p.description,
            ProjectRef::Remote(p) => p.description.as_str(),
        }
    }
    fn stack(&self) -> Vec<String> {
        match self {
            ProjectRef::Static(p) => p.stack.iter().map(|s| s.to_string()).collect(),
            ProjectRef::Remote(p) => p.stack.clone(),
        }
    }
}

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

pub struct Projects {
    stack_filter: Option<String>,
    query: String,
    /// Optional remote override — when Some, rendered instead of the const
    /// fallback. Replaced by the successful response of a "fetch" click.
    remote: Option<Vec<RemoteProject>>,
    /// Shared state with the ehttp callback thread.
    remote_in_flight: Arc<Mutex<RemoteProjectsFetch>>,
    error: Option<String>,
}

#[derive(Debug, Default)]
enum RemoteProjectsFetch {
    #[default]
    Idle,
    Pending,
    Ready(Result<Vec<RemoteProject>, String>),
}

impl Default for Projects {
    fn default() -> Self {
        Self {
            stack_filter: None,
            query: String::new(),
            remote: None,
            remote_in_flight: Arc::new(Mutex::new(RemoteProjectsFetch::Idle)),
            error: None,
        }
    }
}

impl Projects {
    pub fn ui(&mut self, ui: &mut Ui) {
        let accent = palette::accent_now();
        let muted = ui.visuals().weak_text_color();

        // Drain completed fetch.
        self.drain_remote_fetch();
        let fetch_pending = matches!(
            *self.remote_in_flight.lock().unwrap(),
            RemoteProjectsFetch::Pending
        );

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("🔍").weak());
            ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .desired_width(f32::INFINITY)
                    .hint_text("filter projects by name or description"),
            );
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let label = if fetch_pending {
                "⟳ Fetching…"
            } else {
                "☁ Fetch from josh.bot"
            };
            if ui
                .add_enabled(!fetch_pending, egui::Button::new(label))
                .on_hover_text(PROJECTS_REMOTE_URL)
                .clicked()
            {
                let ctx = ui.ctx().clone();
                self.start_remote_fetch(&ctx);
            }
            if self.remote.is_some() {
                ui.label(RichText::new("(live from API)").small().weak());
                if ui.small_button("use local").clicked() {
                    self.remote = None;
                }
            }
        });
        if let Some(err) = &self.error {
            ui.colored_label(Color32::from_rgb(220, 80, 80), err);
        }
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
        let stack_filter = self.stack_filter.clone();
        let refs: Vec<ProjectRef<'_>> = match self.remote.as_ref() {
            Some(remote) => remote.iter().map(ProjectRef::Remote).collect(),
            None => PROJECTS.iter().map(ProjectRef::Static).collect(),
        };
        let matches: Vec<ProjectRef<'_>> = refs
            .into_iter()
            .filter(|p| {
                let text_match = q.is_empty()
                    || p.name().to_lowercase().contains(&q)
                    || p.description().to_lowercase().contains(&q);
                let stack_match = stack_filter
                    .as_deref()
                    .map(|f| p.stack().iter().any(|s| s == f))
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

    fn start_remote_fetch(&mut self, ctx: &egui::Context) {
        {
            let mut state = self.remote_in_flight.lock().unwrap();
            if matches!(*state, RemoteProjectsFetch::Pending) {
                return;
            }
            *state = RemoteProjectsFetch::Pending;
        }
        let store = self.remote_in_flight.clone();
        let ctx = ctx.clone();
        let request = ehttp::Request::get(PROJECTS_REMOTE_URL);
        ehttp::fetch(request, move |result| {
            let parsed: Result<Vec<RemoteProject>, String> = result
                .map_err(|e| e.to_string())
                .and_then(|response| {
                    serde_json::from_slice::<Vec<RemoteProject>>(&response.bytes)
                        .map_err(|e| format!("parse error: {}", e))
                });
            *store.lock().unwrap() = RemoteProjectsFetch::Ready(parsed);
            ctx.request_repaint();
        });
    }

    fn drain_remote_fetch(&mut self) {
        let taken = {
            let mut state = self.remote_in_flight.lock().unwrap();
            if matches!(*state, RemoteProjectsFetch::Ready(_)) {
                Some(std::mem::take(&mut *state))
            } else {
                None
            }
        };
        if let Some(RemoteProjectsFetch::Ready(result)) = taken {
            match result {
                Ok(projects) => {
                    self.remote = Some(projects);
                    self.error = None;
                }
                Err(e) => self.error = Some(format!("fetch failed: {}", e)),
            }
        }
    }
}

fn render_project(
    ui: &mut Ui,
    project: &ProjectRef<'_>,
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
                let name = RichText::new(project.name())
                    .size(16.0)
                    .strong()
                    .color(accent);
                match project.url() {
                    Some(url) => {
                        ui.hyperlink_to(name, url);
                    }
                    None => {
                        ui.label(name);
                    }
                }
            });
            ui.add_space(2.0);
            ui.label(RichText::new(project.description()).color(muted));
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                for tag in project.stack() {
                    let chip = RichText::new(tag.clone())
                        .small()
                        .color(accent)
                        .background_color(accent.linear_multiply(0.15));
                    let resp = ui
                        .add(egui::Label::new(chip).sense(egui::Sense::click()))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text(format!("Filter by {}", tag));
                    if resp.clicked() {
                        *new_filter = Some(tag.clone());
                    }
                }
            });
        });
}
