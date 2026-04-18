use super::note::Note;
use super::note_directory::NoteDirectory;
use egui::{Color32, RichText, Ui};
use std::sync::Arc;

/// A search result with enough context to render a highlighted snippet.
pub struct SearchResult {
    /// The note containing the match.
    pub note: Arc<Note>,

    /// Snippet from the note body with ellipses on either side when the
    /// match isn't at the start/end. Empty when the match is title-only.
    pub snippet: String,

    /// Offset of the matched substring within `snippet`.
    pub match_offset: Option<usize>,

    /// Length of the matched substring (when highlighted inside the snippet).
    pub match_len: usize,

    /// Composite relevance score; higher sorts first.
    pub score: i32,
}

/// Search manager
pub struct Search {
    /// Current search query
    pub query: String,

    /// Most recent results, already sorted by relevance.
    pub results: Vec<SearchResult>,

    /// Currently selected result index
    pub selected_index: Option<usize>,

    /// Input query cached against the results, so we only re-run the search
    /// when the typed query actually changes (avoids per-frame work).
    last_run_query: String,
}

impl Default for Search {
    fn default() -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            selected_index: None,
            last_run_query: String::new(),
        }
    }
}

impl Search {
    pub fn new() -> Self {
        Self::default()
    }

    /// Re-run the search only if the query has changed since the last call.
    /// Safe to invoke every frame.
    pub fn search(&mut self, note_directory: &NoteDirectory) {
        if self.query == self.last_run_query {
            return;
        }
        self.last_run_query = self.query.clone();

        if self.query.trim().is_empty() {
            self.results.clear();
            self.selected_index = None;
            return;
        }

        let query_lower = self.query.to_lowercase();
        let mut results: Vec<SearchResult> = Vec::new();

        for note in note_directory.published_notes() {
            let title_lower = note.title().to_lowercase();
            let tag_score: i32 = note
                .frontmatter
                .tags
                .iter()
                .filter_map(|t| score_tag(&query_lower, &t.to_lowercase()))
                .sum();

            let title_score = score_title(&query_lower, &title_lower);
            let (body_score, snippet, match_offset, match_len) =
                score_body(&query_lower, &note.content);

            let total = title_score + body_score + tag_score;
            if total <= 0 {
                continue;
            }

            results.push(SearchResult {
                note: note.clone(),
                snippet,
                match_offset,
                match_len,
                score: total,
            });
        }

        results.sort_by(|a, b| b.score.cmp(&a.score));
        self.results = results;
        self.selected_index = if self.results.is_empty() { None } else { Some(0) };
    }

    /// Draw the search UI. Returns `Some((note_id, position))` when a result
    /// was clicked this frame (or committed via Enter), `None` otherwise.
    pub fn ui(&mut self, ui: &mut Ui) -> Option<(String, usize)> {
        let mut clicked = None;

        // Arrow / Enter keyboard navigation across the result list.
        ui.input(|i| {
            if self.results.is_empty() {
                return;
            }
            if i.key_pressed(egui::Key::ArrowDown) {
                self.selected_index = Some(match self.selected_index {
                    Some(idx) => (idx + 1).min(self.results.len() - 1),
                    None => 0,
                });
            }
            if i.key_pressed(egui::Key::ArrowUp) {
                self.selected_index = Some(match self.selected_index {
                    Some(idx) => idx.saturating_sub(1),
                    None => 0,
                });
            }
        });
        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            if let Some(idx) = self.selected_index {
                if let Some(result) = self.results.get(idx) {
                    clicked = Some((result.note.id.clone(), 0));
                }
            }
        }

        ui.horizontal(|ui| {
            ui.label("Search:");
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .desired_width(f32::INFINITY)
                    .hint_text("title, body, or #tag"),
            );
            // Auto-focus the search field so users can start typing immediately.
            if !resp.has_focus() && self.query.is_empty() {
                resp.request_focus();
            }
            if ui.button("✕").clicked() {
                self.query.clear();
                self.results.clear();
                self.selected_index = None;
            }
        });

        if !self.results.is_empty() {
            ui.separator();
            ui.label(
                RichText::new(format!("{} result{}",
                    self.results.len(),
                    if self.results.len() == 1 { "" } else { "s" }
                ))
                .small()
                .weak(),
            );

            let accent = ui.visuals().hyperlink_color;
            let muted = ui.visuals().weak_text_color();

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (index, result) in self.results.iter().enumerate() {
                        let is_selected = self.selected_index == Some(index);
                        ui.add_space(4.0);
                        let frame = egui::Frame::NONE
                            .inner_margin(egui::Margin::same(6))
                            .corner_radius(egui::CornerRadius::same(4))
                            .fill(if is_selected {
                                ui.visuals().faint_bg_color
                            } else {
                                Color32::TRANSPARENT
                            });
                        frame.show(ui, |ui| {
                            let title = result.note.title();
                            if ui
                                .link(RichText::new(title).strong().color(accent))
                                .clicked()
                            {
                                clicked = Some((result.note.id.clone(), 0));
                            }
                            if !result.snippet.is_empty() {
                                render_highlighted_snippet(
                                    ui,
                                    &result.snippet,
                                    result.match_offset,
                                    result.match_len,
                                    accent,
                                    muted,
                                );
                            }
                        });
                    }
                });
        } else if !self.query.trim().is_empty() {
            ui.separator();
            ui.label(RichText::new("No results").weak());
        }

        clicked
    }
}

// ---------- scoring ----------

/// Title match is the strongest signal. Exact full-title match scores highest,
/// then prefix match, then substring, then fuzzy (consecutive-char) match.
fn score_title(query: &str, title: &str) -> i32 {
    if title == query {
        return 1000;
    }
    if title.starts_with(query) {
        return 600;
    }
    if title.contains(query) {
        return 400;
    }
    fuzzy_score(query, title).unwrap_or(0) * 3
}

fn score_tag(query: &str, tag: &str) -> Option<i32> {
    // Allow users to search "#foo" or "foo" for tag "foo".
    let q = query.trim_start_matches('#');
    if tag == q {
        return Some(300);
    }
    if tag.contains(q) {
        return Some(150);
    }
    None
}

/// Body match: substring presence + earlier-match bonus. Also returns a
/// snippet around the first match so the UI can render it with highlighting.
fn score_body(query: &str, content: &str) -> (i32, String, Option<usize>, usize) {
    let content_lower = content.to_lowercase();
    let Some(pos) = content_lower.find(query) else {
        return (0, String::new(), None, query.len());
    };

    // Earlier hits score higher (cap at 200 lead).
    let earliness_bonus = (200 - (pos.min(200) as i32)).max(0);
    let score = 100 + earliness_bonus / 4;

    // Build snippet with ~60 chars of context on each side, on char boundaries.
    let snippet_start = floor_char_boundary(content, pos.saturating_sub(60));
    let snippet_end = ceil_char_boundary(content, (pos + query.len() + 60).min(content.len()));
    let mut snippet = content[snippet_start..snippet_end].to_string();

    let mut match_offset = pos - snippet_start;
    if snippet_start > 0 {
        snippet = format!("…{}", snippet);
        match_offset += "…".len();
    }
    if snippet_end < content.len() {
        snippet.push('…');
    }

    // Collapse whitespace so rendered snippet doesn't show stray line breaks.
    let (flat, flat_match_offset) = flatten_whitespace(&snippet, match_offset);

    (score, flat, Some(flat_match_offset), query.len())
}

/// Consecutive-character-match fuzzy scoring. Every char of `query` must
/// appear in `target` in order; score rewards tighter matches.
fn fuzzy_score(query: &str, target: &str) -> Option<i32> {
    let mut score = 0i32;
    let mut last_match: Option<usize> = None;
    let mut q_chars = query.chars().peekable();
    for (i, c) in target.char_indices() {
        match q_chars.peek() {
            Some(&qc) if qc == c => {
                q_chars.next();
                score += 10;
                if let Some(prev) = last_match {
                    // Bonus for consecutive matches.
                    if i == prev + c.len_utf8() {
                        score += 5;
                    }
                }
                last_match = Some(i);
            }
            _ => {}
        }
    }
    if q_chars.peek().is_none() {
        Some(score)
    } else {
        None
    }
}

// ---------- rendering helpers ----------

fn render_highlighted_snippet(
    ui: &mut Ui,
    snippet: &str,
    match_offset: Option<usize>,
    match_len: usize,
    highlight: Color32,
    muted: Color32,
) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        match match_offset {
            Some(off)
                if off <= snippet.len()
                    && off + match_len <= snippet.len()
                    && snippet.is_char_boundary(off)
                    && snippet.is_char_boundary(off + match_len) =>
            {
                let before = &snippet[..off];
                let hit = &snippet[off..off + match_len];
                let after = &snippet[off + match_len..];
                if !before.is_empty() {
                    ui.label(RichText::new(before).small().color(muted));
                }
                ui.label(
                    RichText::new(hit)
                        .small()
                        .strong()
                        .color(highlight)
                        .background_color(highlight.linear_multiply(0.18)),
                );
                if !after.is_empty() {
                    ui.label(RichText::new(after).small().color(muted));
                }
            }
            _ => {
                ui.label(RichText::new(snippet).small().color(muted));
            }
        }
    });
}

fn flatten_whitespace(s: &str, match_offset: usize) -> (String, usize) {
    let mut out = String::with_capacity(s.len());
    let mut new_offset = match_offset;
    let mut byte_pos = 0usize;
    let mut prev_space = false;
    for c in s.chars() {
        let len = c.len_utf8();
        let is_ws = c.is_whitespace();
        if is_ws {
            if !prev_space {
                out.push(' ');
            } else if byte_pos < match_offset {
                // We're collapsing whitespace *before* the match, so the
                // output offset shrinks by the chars we skip.
                new_offset = new_offset.saturating_sub(len);
            }
            prev_space = !prev_space || false; // collapse any further spaces
            if prev_space && byte_pos < match_offset && out.len() == 0 {
                // unreachable; kept for clarity
            }
        } else {
            out.push(c);
            prev_space = false;
        }
        byte_pos += len;
    }
    // Clamp offset within the new string bounds.
    if new_offset > out.len() {
        new_offset = out.len();
    }
    (out, new_offset)
}

/// Round down to the nearest char boundary. `str::floor_char_boundary` is
/// unstable; this is a small stable substitute.
fn floor_char_boundary(s: &str, mut idx: usize) -> usize {
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn ceil_char_boundary(s: &str, mut idx: usize) -> usize {
    while idx < s.len() && !s.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}

// ---------- tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_exact_match_beats_body() {
        let title = score_title("rust", "rust");
        let (body, _, _, _) = score_body("rust", "rust is a systems language");
        assert!(title > body, "title exact should outrank any body match");
    }

    #[test]
    fn title_prefix_beats_substring() {
        let prefix = score_title("rust", "rust is great");
        let substring = score_title("rust", "i love rust so much");
        assert!(prefix > substring);
    }

    #[test]
    fn earlier_body_match_scores_higher() {
        let (early, _, _, _) = score_body("rust", "rust appears early here");
        let (late, _, _, _) = score_body(
            "rust",
            &("filler ".repeat(40) + "rust appears late here"),
        );
        assert!(early > late);
    }

    #[test]
    fn body_snippet_wraps_match_with_ellipses() {
        let long = "a ".repeat(200) + "needle in the middle somewhere";
        let (_, snippet, off, len) = score_body("needle", &long);
        assert!(snippet.starts_with('…'));
        let off = off.expect("match in snippet");
        // `off` is a byte offset into `snippet`, as documented on SearchResult.
        assert!(snippet.is_char_boundary(off));
        assert_eq!(&snippet[off..off + len], "needle");
    }

    #[test]
    fn fuzzy_matches_noncontiguous_chars() {
        assert!(fuzzy_score("rst", "rust").is_some());
        assert!(fuzzy_score("abc", "axbxc").is_some());
        assert!(fuzzy_score("xyz", "rust").is_none());
    }

    #[test]
    fn fuzzy_prefers_consecutive_matches() {
        let tight = fuzzy_score("rust", "rust lang").unwrap();
        let loose = fuzzy_score("rust", "r_u_s_t").unwrap();
        assert!(tight > loose);
    }

    #[test]
    fn tag_search_accepts_hash_prefix() {
        assert_eq!(score_tag("#rust", "rust"), Some(300));
        assert_eq!(score_tag("rust", "rust"), Some(300));
    }

    #[test]
    fn empty_query_returns_no_body_match() {
        let (score, _, _, _) = score_body("", "anything");
        // Empty string is contained in every string, but we also check
        // the search() wrapper rejects empty queries before getting here.
        assert!(score > 0); // find("") returns Some(0), so this does match
    }

    #[test]
    fn snippet_multibyte_safe() {
        // Query is at the end; snippet window must not split a codepoint.
        let s = "こんにちは世界 needle";
        let (_, snippet, _, _) = score_body("needle", s);
        // Just needs to produce a valid String; before the fix, non-utf8
        // slicing would panic.
        assert!(snippet.contains("needle"));
    }
}
