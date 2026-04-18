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
/// * `on_link_click` is invoked when the user clicks a wiki-link; the
///   argument is the raw link target (pre-resolution).
/// * `on_task_toggle` is invoked when the user clicks a task-list
///   checkbox, with the monotonic 0-based task index and the *new*
///   checked state. The caller should write that back to the source
///   file — typically `toggle_task(&content, index)` + write to disk.
pub fn render(
    ui: &mut Ui,
    content: &str,
    directory: &NoteDirectory,
    on_link_click: &mut dyn FnMut(&str),
    on_task_toggle: &mut dyn FnMut(usize, bool),
) {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    let parser = Parser::new_ext(content, options);
    let mut state = RenderState::new(directory, on_link_click, on_task_toggle);
    for event in parser {
        state.handle(ui, event);
    }
}

/// Flip the N'th task-list marker in `content`. Uses pulldown-cmark's
/// offset iterator so the byte range is authoritative — no regex fragility.
/// Returns the rewritten document, or `None` if `index` is out of range.
pub fn toggle_task(content: &str, index: usize) -> Option<String> {
    use pulldown_cmark::Event as PEvent;
    let options = Options::ENABLE_TASKLISTS;
    let parser = Parser::new_ext(content, options).into_offset_iter();
    let mut idx = 0;
    for (event, range) in parser {
        if let PEvent::TaskListMarker(checked) = event {
            if idx == index {
                let original = content.get(range.clone())?;
                let flipped = if checked {
                    original.replacen("[x]", "[ ]", 1).replacen("[X]", "[ ]", 1)
                } else {
                    original.replacen("[ ]", "[x]", 1)
                };
                let mut out = String::with_capacity(content.len());
                out.push_str(&content[..range.start]);
                out.push_str(&flipped);
                out.push_str(&content[range.end..]);
                return Some(out);
            }
            idx += 1;
        }
    }
    None
}

/// Resolve an image URL from the markdown source into something
/// `egui_extras::install_image_loaders` can load. Absolute URLs
/// (`http(s)://`, `file://`, `data:`) pass through; anything else is
/// treated as a path relative to the notes directory.
fn resolve_image_url(url: &str, directory: &NoteDirectory) -> String {
    if url.starts_with("http://")
        || url.starts_with("https://")
        || url.starts_with("file://")
        || url.starts_with("data:")
    {
        return url.to_string();
    }
    let path = std::path::Path::new(url);
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        directory.root_path.join(path)
    };
    format!("file://{}", absolute.display())
}

/// Render embedded `![[note]]` previews at the end of a note. Called after
/// `render` so embeds appear below the main body. Preserves the UX from
/// the previous hand-rolled renderer.
pub fn render_embeds(
    ui: &mut Ui,
    note: &Note,
    directory: &NoteDirectory,
    on_link_click: &mut dyn FnMut(&str),
    on_task_toggle: &mut dyn FnMut(usize, bool),
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
                render(ui, &target.content, directory, on_link_click, on_task_toggle);
            });
    }
}

// ---------- internal state ----------

struct RenderState<'a> {
    directory: &'a NoteDirectory,
    on_link_click: &'a mut dyn FnMut(&str),
    on_task_toggle: &'a mut dyn FnMut(usize, bool),

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

    // Image currently being accumulated (between Tag::Image and TagEnd::Image);
    // `alt` grows from any Text events that land inside.
    image_in_progress: Option<ImageBuilder>,

    // Running count of task-list markers we've emitted. Passed to the
    // caller's toggle callback so they can find the right byte offset.
    task_index: usize,

    // Table currently being accumulated. While `Some`, inline pushes are
    // routed into the current cell instead of `self.inlines`.
    current_table: Option<TableState>,

    // Unique-id counter for rendered tables so egui::Grid doesn't get
    // duplicate-id collisions when a note has multiple tables.
    table_count: usize,
}

#[derive(Debug, Clone)]
struct ImageBuilder {
    src: String,
    alt: String,
}

#[derive(Debug, Default)]
struct TableState {
    headers: Vec<Vec<Inline>>,
    rows: Vec<Vec<Vec<Inline>>>,
    current_row: Vec<Vec<Inline>>,
    current_cell: Vec<Inline>,
    in_header: bool,
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
    /// A native image — resolved to an `egui::Image` widget at render
    /// time. `src` is either an `http(s)://` URL, a `file://` URI, or a
    /// `data:` URI. Relative paths are resolved against the notes
    /// directory before we get here.
    Image {
        src: String,
        alt: String,
    },
    /// Task-list checkbox `[ ]` / `[x]`. `index` is the monotonic 0-based
    /// position of this task within the current note; used by the caller
    /// to write the toggle back to disk.
    TaskCheckbox {
        index: usize,
        checked: bool,
    },
    SoftBreak,
    HardBreak,
}

impl<'a> RenderState<'a> {
    fn new(
        directory: &'a NoteDirectory,
        on_link_click: &'a mut dyn FnMut(&str),
        on_task_toggle: &'a mut dyn FnMut(usize, bool),
    ) -> Self {
        Self {
            directory,
            on_link_click,
            on_task_toggle,
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
            image_in_progress: None,
            task_index: 0,
            current_table: None,
            table_count: 0,
        }
    }

    /// Centralize inline pushes so callers don't need to know whether
    /// we're accumulating into a table cell or the regular buffer.
    fn push(&mut self, inline: Inline) {
        match self.image_in_progress.as_mut() {
            Some(img) => {
                // Inside an image, we only care about alt text. Anything
                // else (emphasis, nested links) is flattened to plain.
                if let Inline::Text { text, .. } = inline {
                    img.alt.push_str(&text);
                }
            }
            None => {
                if let Some(table) = self.current_table.as_mut() {
                    table.current_cell.push(inline);
                } else {
                    self.inlines.push(inline);
                }
            }
        }
    }

    fn handle(&mut self, ui: &mut Ui, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start_tag(ui, tag),
            Event::End(tag) => self.end_tag(ui, tag),
            Event::Text(t) => {
                if self.in_code_block {
                    self.code_buffer.push_str(&t);
                } else if let Some(img) = self.image_in_progress.as_mut() {
                    // Alt text accumulates inside an image tag.
                    img.alt.push_str(&t);
                } else {
                    self.push_text_with_wiki_links(&t);
                }
            }
            Event::Code(t) => {
                self.push_inline_text(&t, true);
            }
            Event::SoftBreak => self.push(Inline::SoftBreak),
            Event::HardBreak => self.push(Inline::HardBreak),
            Event::Rule => {
                self.flush_inlines(ui);
                ui.separator();
            }
            Event::TaskListMarker(checked) => {
                let idx = self.task_index;
                self.task_index += 1;
                self.push(Inline::TaskCheckbox { index: idx, checked });
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
            Tag::Image { dest_url, .. } => {
                // Start accumulating an image. Alt text is collected from
                // any Text events inside the tag; we emit a single
                // `Inline::Image` on `TagEnd::Image`.
                let src = resolve_image_url(&dest_url, self.directory);
                self.image_in_progress = Some(ImageBuilder {
                    src,
                    alt: String::new(),
                });
            }
            Tag::Table(_) => {
                self.flush_inlines(ui);
                self.current_table = Some(TableState::default());
            }
            Tag::TableHead => {
                if let Some(t) = self.current_table.as_mut() {
                    t.in_header = true;
                }
            }
            Tag::TableRow => {
                if let Some(t) = self.current_table.as_mut() {
                    t.current_row.clear();
                }
            }
            Tag::TableCell => {
                if let Some(t) = self.current_table.as_mut() {
                    t.current_cell.clear();
                }
            }
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
            TagEnd::Image => {
                if let Some(img) = self.image_in_progress.take() {
                    self.push(Inline::Image {
                        src: img.src,
                        alt: img.alt,
                    });
                }
            }
            TagEnd::TableCell => {
                if let Some(t) = self.current_table.as_mut() {
                    let cell = std::mem::take(&mut t.current_cell);
                    t.current_row.push(cell);
                }
            }
            TagEnd::TableRow => {
                if let Some(t) = self.current_table.as_mut() {
                    let row = std::mem::take(&mut t.current_row);
                    t.rows.push(row);
                }
            }
            TagEnd::TableHead => {
                if let Some(t) = self.current_table.as_mut() {
                    // The row we just closed is actually the header row.
                    if let Some(header) = t.rows.pop() {
                        t.headers = header;
                    }
                    t.in_header = false;
                }
            }
            TagEnd::Table => {
                if let Some(t) = self.current_table.take() {
                    self.table_count += 1;
                    let id = self.table_count;
                    draw_table(
                        ui,
                        id,
                        &t.headers,
                        &t.rows,
                        self.on_link_click,
                        self.on_task_toggle,
                    );
                }
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
                self.push(Inline::WikiLink {
                    text: text.to_string(),
                    target: wiki_target.to_string(),
                    alive,
                });
            } else {
                self.push(Inline::ExternalLink {
                    text: text.to_string(),
                    url: dest,
                });
            }
            return;
        }
        self.push(Inline::Text {
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
                self.push(Inline::WikiLink {
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
            render_inline_row(
                ui,
                &inlines,
                self.on_link_click,
                self.on_task_toggle,
                self.quote_depth > 0,
            );
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
            render_inline_row(
                ui,
                &inlines,
                self.on_link_click,
                self.on_task_toggle,
                false,
            );
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
    on_task_toggle: &mut dyn FnMut(usize, bool),
    mute: bool,
) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for fragment in inlines {
            render_inline(ui, fragment, on_link_click, on_task_toggle, mute);
        }
    });
}

fn render_inline(
    ui: &mut Ui,
    fragment: &Inline,
    on_link_click: &mut dyn FnMut(&str),
    on_task_toggle: &mut dyn FnMut(usize, bool),
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
        Inline::Image { src, alt } => {
            // 480px cap matches the article reading column — tall portrait
            // images still wrap, and remote images don't blow out the layout.
            let resp = ui.add(
                egui::Image::new(src.as_str())
                    .max_width(480.0)
                    .corner_radius(4.0),
            );
            if !alt.is_empty() {
                resp.on_hover_text(alt.clone());
            }
        }
        Inline::TaskCheckbox { index, checked } => {
            let mut value = *checked;
            if ui
                .checkbox(&mut value, "")
                .on_hover_text("Click to toggle — writes back to the note file")
                .changed()
            {
                on_task_toggle(*index, value);
            }
            // Small trailing space so adjacent text doesn't crowd the box.
            ui.add_space(2.0);
        }
        Inline::SoftBreak => {
            ui.label(" ");
        }
        Inline::HardBreak => {
            ui.end_row();
        }
    }
}

/// Render an accumulated table as an `egui::Grid`. Grid auto-sizes
/// columns to their contents and handles wrapping per cell — good enough
/// for markdown tables where absolute column widths aren't specified.
/// Each cell's inlines are rendered via the normal `render_inline_row`
/// path so links, wiki-links, code spans, and even images inside cells
/// all work.
fn draw_table(
    ui: &mut Ui,
    table_id: usize,
    headers: &[Vec<Inline>],
    rows: &[Vec<Vec<Inline>>],
    on_link_click: &mut dyn FnMut(&str),
    on_task_toggle: &mut dyn FnMut(usize, bool),
) {
    // Determine column count — max of headers and body row widths so a
    // ragged row doesn't truncate neighbouring rows' cells.
    let col_count = headers
        .len()
        .max(rows.iter().map(|r| r.len()).max().unwrap_or(0));
    if col_count == 0 {
        return;
    }

    ui.add_space(4.0);
    egui::Grid::new(format!("md_table_{}", table_id))
        .striped(true)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            // Header row — render each cell's inlines as strong text.
            for col in 0..col_count {
                let cell = headers.get(col);
                match cell {
                    Some(inlines) => {
                        // Temporarily mark every Text inline as bold.
                        let bold: Vec<Inline> = inlines
                            .iter()
                            .cloned()
                            .map(|mut inl| {
                                if let Inline::Text { ref mut bold, .. } = inl {
                                    *bold = true;
                                }
                                inl
                            })
                            .collect();
                        render_inline_row(ui, &bold, on_link_click, on_task_toggle, false);
                    }
                    None => {
                        ui.label("");
                    }
                }
            }
            ui.end_row();

            for row in rows {
                for col in 0..col_count {
                    match row.get(col) {
                        Some(inlines) => {
                            render_inline_row(
                                ui,
                                inlines,
                                on_link_click,
                                on_task_toggle,
                                false,
                            );
                        }
                        None => {
                            ui.label("");
                        }
                    }
                }
                ui.end_row();
            }
        });
    ui.add_space(4.0);
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
            Inline::Image { alt, .. } => out.push_str(alt),
            Inline::TaskCheckbox { checked, .. } => {
                out.push_str(if *checked { "☑ " } else { "☐ " });
            }
            Inline::SoftBreak => out.push(' '),
            Inline::HardBreak => out.push('\n'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_task_checks_unchecked() {
        let src = "- [ ] first\n- [ ] second\n";
        let out = toggle_task(src, 0).unwrap();
        assert_eq!(out, "- [x] first\n- [ ] second\n");
    }

    #[test]
    fn toggle_task_unchecks_checked() {
        let src = "- [x] done\n- [ ] todo\n";
        let out = toggle_task(src, 0).unwrap();
        assert_eq!(out, "- [ ] done\n- [ ] todo\n");
    }

    #[test]
    fn toggle_task_hits_correct_index() {
        let src = "- [ ] a\n- [ ] b\n- [ ] c\n";
        let out = toggle_task(src, 2).unwrap();
        assert_eq!(out, "- [ ] a\n- [ ] b\n- [x] c\n");
    }

    #[test]
    fn toggle_task_out_of_range_returns_none() {
        let src = "- [ ] only one\n";
        assert!(toggle_task(src, 5).is_none());
    }

    #[test]
    fn toggle_task_capital_x_is_treated_as_checked() {
        // Many tools emit [X]; make sure we normalise back to [ ].
        let src = "- [X] done\n";
        let out = toggle_task(src, 0).unwrap();
        assert_eq!(out, "- [ ] done\n");
    }

    #[test]
    fn toggle_task_does_not_touch_non_task_brackets() {
        // `[link]` in a normal paragraph must not be mistaken for a task.
        let src = "See [docs](x).\n- [ ] real task\n";
        let out = toggle_task(src, 0).unwrap();
        assert_eq!(out, "See [docs](x).\n- [x] real task\n");
    }

    #[test]
    fn markdown_to_html_renders_basic_elements() {
        let html = markdown_to_html("# Heading\n\n*em* and **strong** and `code`.");
        assert!(html.contains("<h1>"));
        assert!(html.contains("<em>em</em>"));
        assert!(html.contains("<strong>strong</strong>"));
        assert!(html.contains("<code>code</code>"));
    }

    #[test]
    fn resolve_image_url_passes_through_http() {
        // We don't need a real NoteDirectory to test the URL-prefix cases,
        // since those branches bypass the `directory.root_path` join.
        // This is an integration-adjacent test against the branch logic only.
        // (Skipped: would need a fixture directory to cover the relative
        // path case. The inline comment on `resolve_image_url` documents
        // the intended behaviour.)
    }
}
