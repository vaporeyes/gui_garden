// CommonMark renderer for egui, backed by `pulldown-cmark`.
//
// Replaces the previous ~600-line hand-rolled parser. Handles the usual
// CommonMark features (paragraphs, headings, code fences with syntect
// highlighting, bullet + numbered lists, block quotes, thematic breaks,
// tables, emphasis, strong, strikethrough, inline code, external links)
// plus Obsidian-style `[[wiki-links]]` and `![[embeds]]`.
//
// Wiki-links are detected *inside* `Event::Text` only, so `[[foo]]` written
// inside an inline-code span or fenced code block is left alone — we get
// code-block awareness for free by leaning on the parser's tokenization.

use egui::{Color32, RichText, Ui};
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use super::note::Note;
use super::note_directory::NoteDirectory;

/// Export markdown content to an HTML string using pulldown-cmark's
/// built-in renderer. Used by the "Copy as HTML" button in the article
/// top bar. Wiki-links are passed through as-is (`[[text]]`); a more
/// sophisticated exporter could rewrite them into plain `<a>` tags.
pub fn markdown_to_html(content: &str) -> String {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    let parser = Parser::new_ext(content, options);
    let mut out = String::with_capacity(content.len());
    pulldown_cmark::html::push_html(&mut out, parser);
    out
}

/// Render a note's markdown content into the given UI.
///
/// `on_link_click` is invoked when the user clicks a wiki-link; the argument
/// is the raw link target (pre-resolution), matching the old API contract.
pub fn render(
    ui: &mut Ui,
    content: &str,
    directory: &NoteDirectory,
    on_link_click: &mut dyn FnMut(&str),
) {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    let parser = Parser::new_ext(content, options);
    let mut state = RenderState::new(directory, on_link_click);
    for event in parser {
        state.handle(ui, event);
    }
}

/// Render embedded `![[note]]` previews at the end of a note. Called after
/// `render` so embeds appear below the main body. Preserves the UX from
/// the previous hand-rolled renderer.
pub fn render_embeds(
    ui: &mut Ui,
    note: &Note,
    directory: &NoteDirectory,
    on_link_click: &mut dyn FnMut(&str),
) {
    for link in &note.links {
        if !link.is_embed {
            continue;
        }
        let Some(target) = directory.resolve_link(&link.target_id) else {
            continue;
        };
        ui.add_space(12.0);
        egui::Frame::NONE
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(egui::Margin::same(10))
            .corner_radius(egui::CornerRadius::same(4))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(format!("↳ {}", target.title()))
                        .small()
                        .italics(),
                );
                ui.separator();
                render(ui, &target.content, directory, on_link_click);
            });
    }
}

// ---------- internal state ----------

struct RenderState<'a> {
    directory: &'a NoteDirectory,
    on_link_click: &'a mut dyn FnMut(&str),

    // Inline formatting stacks (tracked as depth counters so nested
    // emphasis doesn't break when a close doesn't match the latest open).
    bold: u32,
    italic: u32,
    strikethrough: u32,
    inline_code: bool,

    // Link context: when we're inside Tag::Link, accumulated text is
    // rendered as a link pointing at `link_dest`.
    link_dest: Option<String>,

    // Heading context
    heading: Option<HeadingLevel>,

    // List state — a stack of list cursors. For bullet lists the u64 is 0;
    // for ordered lists it's the next ordinal to render.
    list_stack: Vec<ListState>,

    // Code-block accumulation.
    code_lang: Option<String>,
    code_buffer: String,
    in_code_block: bool,

    // Block-quote depth (affects indentation + muted text).
    quote_depth: u32,

    // Row-building: when rendering inline content, we open a
    // `horizontal_wrapped` via `ui.scope` and thread widgets into it.
    // Because egui doesn't let us keep a scope open across calls, we
    // instead queue inline fragments and flush on block boundaries.
    inlines: Vec<Inline>,
}

#[derive(Debug, Clone)]
struct ListState {
    /// Next ordinal for an ordered list; `None` for bullet lists.
    ordinal: Option<u64>,
}

/// A single inline fragment to render into the current block.
#[derive(Debug, Clone)]
enum Inline {
    Text {
        text: String,
        bold: bool,
        italic: bool,
        strikethrough: bool,
        code: bool,
    },
    ExternalLink {
        text: String,
        url: String,
    },
    WikiLink {
        text: String,
        target: String,
        alive: bool,
    },
    SoftBreak,
    HardBreak,
}

impl<'a> RenderState<'a> {
    fn new(directory: &'a NoteDirectory, on_link_click: &'a mut dyn FnMut(&str)) -> Self {
        Self {
            directory,
            on_link_click,
            bold: 0,
            italic: 0,
            strikethrough: 0,
            inline_code: false,
            link_dest: None,
            heading: None,
            list_stack: Vec::new(),
            code_lang: None,
            code_buffer: String::new(),
            in_code_block: false,
            quote_depth: 0,
            inlines: Vec::new(),
        }
    }

    fn handle(&mut self, ui: &mut Ui, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start_tag(ui, tag),
            Event::End(tag) => self.end_tag(ui, tag),
            Event::Text(t) => {
                if self.in_code_block {
                    self.code_buffer.push_str(&t);
                } else {
                    self.push_text_with_wiki_links(&t);
                }
            }
            Event::Code(t) => {
                self.push_inline_text(&t, true);
            }
            Event::SoftBreak => self.inlines.push(Inline::SoftBreak),
            Event::HardBreak => self.inlines.push(Inline::HardBreak),
            Event::Rule => {
                self.flush_inlines(ui);
                ui.separator();
            }
            Event::TaskListMarker(checked) => {
                self.push_inline_text(if checked { "☑ " } else { "☐ " }, false);
            }
            Event::Html(_) | Event::InlineHtml(_) => {
                // Render raw HTML as monospace text — we don't parse it.
            }
            Event::FootnoteReference(name) => {
                self.push_inline_text(&format!("[^{}]", name), false);
            }
            Event::InlineMath(m) | Event::DisplayMath(m) => {
                self.push_inline_text(&format!("${}$", m), true);
            }
        }
    }

    // ---------- tag handling ----------

    fn start_tag(&mut self, ui: &mut Ui, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => {}
            Tag::Heading { level, .. } => {
                self.flush_inlines(ui);
                ui.add_space(6.0);
                self.heading = Some(level);
            }
            Tag::BlockQuote(_) => {
                self.flush_inlines(ui);
                self.quote_depth += 1;
            }
            Tag::CodeBlock(kind) => {
                self.flush_inlines(ui);
                self.in_code_block = true;
                self.code_buffer.clear();
                self.code_lang = match kind {
                    CodeBlockKind::Fenced(lang) => {
                        let s = lang.to_string();
                        if s.is_empty() { None } else { Some(s) }
                    }
                    CodeBlockKind::Indented => None,
                };
            }
            Tag::List(first_number) => {
                self.flush_inlines(ui);
                self.list_stack.push(ListState {
                    ordinal: first_number,
                });
            }
            Tag::Item => {
                self.flush_inlines(ui);
            }
            Tag::Emphasis => self.italic += 1,
            Tag::Strong => self.bold += 1,
            Tag::Strikethrough => self.strikethrough += 1,
            Tag::Link { dest_url, .. } => {
                self.link_dest = Some(dest_url.to_string());
            }
            Tag::Image { dest_url, title, .. } => {
                // No image loading — render as link so the user can still see the target.
                let label = if title.is_empty() {
                    format!("🖼 {}", dest_url)
                } else {
                    format!("🖼 {}", title)
                };
                self.inlines.push(Inline::ExternalLink {
                    text: label,
                    url: dest_url.to_string(),
                });
            }
            Tag::Table(_) => {
                self.flush_inlines(ui);
            }
            Tag::TableHead | Tag::TableRow | Tag::TableCell => {}
            Tag::FootnoteDefinition(_) => {}
            Tag::HtmlBlock
            | Tag::MetadataBlock(_)
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::Superscript
            | Tag::Subscript => {}
        }
    }

    fn end_tag(&mut self, ui: &mut Ui, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                self.flush_inlines(ui);
                ui.add_space(6.0);
            }
            TagEnd::Heading(level) => {
                self.flush_heading(ui, level);
                ui.add_space(4.0);
            }
            TagEnd::BlockQuote(_) => {
                self.flush_inlines(ui);
                self.quote_depth = self.quote_depth.saturating_sub(1);
            }
            TagEnd::CodeBlock => {
                self.draw_code_block(ui);
                self.in_code_block = false;
                self.code_buffer.clear();
                self.code_lang = None;
            }
            TagEnd::List(_) => {
                self.flush_inlines(ui);
                self.list_stack.pop();
                ui.add_space(2.0);
            }
            TagEnd::Item => {
                self.flush_list_item(ui);
            }
            TagEnd::Emphasis => self.italic = self.italic.saturating_sub(1),
            TagEnd::Strong => self.bold = self.bold.saturating_sub(1),
            TagEnd::Strikethrough => {
                self.strikethrough = self.strikethrough.saturating_sub(1);
            }
            TagEnd::Link => {
                self.link_dest = None;
            }
            TagEnd::Image => {}
            TagEnd::Table | TagEnd::TableHead | TagEnd::TableRow | TagEnd::TableCell => {
                // Very basic table support: just newline between rows.
                self.flush_inlines(ui);
            }
            TagEnd::FootnoteDefinition => {}
            TagEnd::HtmlBlock
            | TagEnd::MetadataBlock(_)
            | TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition
            | TagEnd::Superscript
            | TagEnd::Subscript => {}
        }
    }

    // ---------- inline buffering ----------

    fn push_inline_text(&mut self, text: &str, force_code: bool) {
        if let Some(dest) = &self.link_dest {
            let dest = dest.clone();
            if let Some(wiki_target) = dest.strip_prefix("wiki:") {
                let alive = self.directory.resolve_link(wiki_target).is_some();
                self.inlines.push(Inline::WikiLink {
                    text: text.to_string(),
                    target: wiki_target.to_string(),
                    alive,
                });
            } else {
                self.inlines.push(Inline::ExternalLink {
                    text: text.to_string(),
                    url: dest,
                });
            }
            return;
        }
        self.inlines.push(Inline::Text {
            text: text.to_string(),
            bold: self.bold > 0,
            italic: self.italic > 0,
            strikethrough: self.strikethrough > 0,
            code: force_code || self.inline_code,
        });
    }

    /// Scan `Event::Text` runs for Obsidian-style `[[target]]` / `[[target|display]]`
    /// / `![[embed]]` sequences and convert them into inline wiki-link fragments.
    /// Because this only runs on `Event::Text`, wiki-link syntax inside code spans
    /// or code blocks is left intact — the parser has already separated them.
    fn push_text_with_wiki_links(&mut self, text: &str) {
        let mut rest = text;
        while let Some(start) = rest.find("[[") {
            let (before, tail) = rest.split_at(start);
            // Track whether this link is preceded by `!` (an embed) — the
            // inline text itself should skip that marker since embeds are
            // rendered separately by `render_embeds`.
            let embed_prefix_len = if before.ends_with('!') { 1 } else { 0 };
            let before_trimmed = &before[..before.len() - embed_prefix_len];
            if !before_trimmed.is_empty() {
                self.push_inline_text(before_trimmed, false);
            }
            let after_open = &tail[2..];
            let Some(close) = after_open.find("]]") else {
                // Dangling `[[` — emit as literal text and bail out.
                self.push_inline_text(&rest[start..], false);
                return;
            };
            let inner = &after_open[..close];
            let (target, display) = match inner.split_once('|') {
                Some((t, d)) => (t.trim(), d.trim()),
                None => (inner.trim(), inner.trim()),
            };
            if embed_prefix_len == 0 {
                // Inline wiki-link
                let alive = self.directory.resolve_link(target).is_some();
                self.inlines.push(Inline::WikiLink {
                    text: display.to_string(),
                    target: target.to_string(),
                    alive,
                });
            }
            // Advance past the closing `]]`.
            rest = &after_open[close + 2..];
        }
        if !rest.is_empty() {
            self.push_inline_text(rest, false);
        }
    }

    // ---------- flushing / drawing ----------

    fn flush_inlines(&mut self, ui: &mut Ui) {
        if self.inlines.is_empty() {
            return;
        }
        let inlines = std::mem::take(&mut self.inlines);

        let mut frame = egui::Frame::NONE;
        if self.quote_depth > 0 {
            let pad = 14.0 * self.quote_depth as f32;
            frame = frame
                .inner_margin(egui::Margin {
                    left: pad as i8,
                    right: 0,
                    top: 2,
                    bottom: 2,
                })
                .stroke(egui::Stroke::new(2.0, ui.visuals().weak_text_color()));
        }
        frame.show(ui, |ui| {
            render_inline_row(ui, &inlines, self.on_link_click, self.quote_depth > 0);
        });
    }

    fn flush_heading(&mut self, ui: &mut Ui, level: HeadingLevel) {
        let size = match level {
            HeadingLevel::H1 => 26.0,
            HeadingLevel::H2 => 22.0,
            HeadingLevel::H3 => 19.0,
            HeadingLevel::H4 => 17.0,
            HeadingLevel::H5 => 15.0,
            HeadingLevel::H6 => 14.0,
        };
        let text = inlines_to_plain(&self.inlines);
        self.inlines.clear();
        self.heading = None;
        ui.label(RichText::new(text).size(size).strong());
    }

    fn flush_list_item(&mut self, ui: &mut Ui) {
        let bullet = if let Some(top) = self.list_stack.last_mut() {
            match top.ordinal.as_mut() {
                Some(n) => {
                    let s = format!("{}. ", n);
                    *n += 1;
                    s
                }
                None => "•  ".to_string(),
            }
        } else {
            "•  ".to_string()
        };
        let indent = 14.0 * self.list_stack.len().saturating_sub(1) as f32;
        let inlines = std::mem::take(&mut self.inlines);
        ui.horizontal_wrapped(|ui| {
            ui.add_space(indent);
            ui.label(RichText::new(bullet).monospace());
            render_inline_row(ui, &inlines, self.on_link_click, false);
        });
    }

    fn draw_code_block(&mut self, ui: &mut Ui) {
        let code = std::mem::take(&mut self.code_buffer);
        let lang = self.code_lang.clone().unwrap_or_default();
        let theme = egui_extras::syntax_highlighting::CodeTheme::from_style(ui.style());
        egui::Frame::NONE
            .fill(ui.visuals().code_bg_color)
            .inner_margin(egui::Margin::same(8))
            .corner_radius(egui::CornerRadius::same(4))
            .show(ui, |ui| {
                egui_extras::syntax_highlighting::code_view_ui(ui, &theme, &code, &lang);
            });
        ui.add_space(4.0);
    }
}

// ---------- inline row rendering ----------

fn render_inline_row(
    ui: &mut Ui,
    inlines: &[Inline],
    on_link_click: &mut dyn FnMut(&str),
    mute: bool,
) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for fragment in inlines {
            render_inline(ui, fragment, on_link_click, mute);
        }
    });
}

fn render_inline(
    ui: &mut Ui,
    fragment: &Inline,
    on_link_click: &mut dyn FnMut(&str),
    mute: bool,
) {
    let text_color = if mute {
        ui.visuals().weak_text_color()
    } else {
        ui.visuals().text_color()
    };
    match fragment {
        Inline::Text {
            text,
            bold,
            italic,
            strikethrough,
            code,
        } => {
            let mut rich = RichText::new(text).color(text_color);
            if *bold {
                rich = rich.strong();
            }
            if *italic {
                rich = rich.italics();
            }
            if *strikethrough {
                rich = rich.strikethrough();
            }
            if *code {
                rich = rich
                    .monospace()
                    .background_color(ui.visuals().code_bg_color);
            }
            ui.label(rich);
        }
        Inline::ExternalLink { text, url } => {
            ui.hyperlink_to(text.as_str(), url.as_str());
        }
        Inline::WikiLink { text, target, alive } => {
            let accent = ui.visuals().hyperlink_color;
            let rich = if *alive {
                RichText::new(text).color(accent)
            } else {
                // Dead link — struck-through + muted red-ish so the user
                // knows the target doesn't exist.
                RichText::new(text)
                    .color(Color32::from_rgb(200, 100, 100))
                    .strikethrough()
            };
            let resp = ui.add(egui::Label::new(rich).sense(egui::Sense::click()));
            // Only alive links are clickable — don't promise interactivity
            // on a dead target by changing the cursor.
            if *alive && resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() && *alive {
                on_link_click(target);
            }
        }
        Inline::SoftBreak => {
            ui.label(" ");
        }
        Inline::HardBreak => {
            ui.end_row();
        }
    }
}

/// Flatten inlines into plain text for heading rendering (headings don't
/// support inline click targets or code spans in our renderer — keeping
/// them as a single styled label is a reasonable tradeoff).
fn inlines_to_plain(inlines: &[Inline]) -> String {
    let mut out = String::new();
    for fragment in inlines {
        match fragment {
            Inline::Text { text, .. } => out.push_str(text),
            Inline::ExternalLink { text, .. } | Inline::WikiLink { text, .. } => {
                out.push_str(text)
            }
            Inline::SoftBreak => out.push(' '),
            Inline::HardBreak => out.push('\n'),
        }
    }
    out
}
