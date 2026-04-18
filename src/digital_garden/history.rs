// Browser-style note navigation history with bounded size.
//
// Extracted from `DigitalGarden` to isolate the bounds-checking and
// capping logic — both of which were previously inlined and latent-buggy
// (underflow on empty history, unbounded growth, no dedupe on chained pushes).

const HISTORY_CAP: usize = 100;

#[derive(Debug, Clone)]
pub struct History {
    entries: Vec<String>,
    position: usize,
}

impl Default for History {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            position: 0,
        }
    }
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the whole history with a single entry, positioned at it.
    /// Used when a new notes directory is loaded — we want a clean slate,
    /// not leftover `history_position` from a prior session.
    pub fn reset(&mut self, id: String) {
        self.entries = vec![id];
        self.position = 0;
    }

    /// Drop every entry and reset the cursor.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.position = 0;
    }

    pub fn current(&self) -> Option<&str> {
        self.entries.get(self.position).map(|s| s.as_str())
    }

    /// The `n` most recently visited distinct ids, most-recent first,
    /// excluding the current entry. Used by the sidebar's "Recent" section.
    pub fn recent(&self, n: usize) -> Vec<String> {
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        if let Some(cur) = self.current() {
            seen.insert(cur);
        }
        let mut out: Vec<String> = Vec::with_capacity(n);
        // Walk backwards from the cursor through the history timeline.
        for id in self.entries[..self.position].iter().rev() {
            if seen.insert(id.as_str()) {
                out.push(id.clone());
                if out.len() >= n {
                    break;
                }
            }
        }
        out
    }

    pub fn can_go_back(&self) -> bool {
        self.position > 0 && !self.entries.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.entries.is_empty() && self.position + 1 < self.entries.len()
    }

    /// Push a new entry at the current cursor, mimicking browser semantics:
    ///   - If the cursor isn't at the tail, future entries are truncated.
    ///   - Consecutive duplicates (same id as current) are skipped.
    ///   - Size is capped; overflow drops from the front.
    pub fn push(&mut self, id: String) {
        if self.current() == Some(id.as_str()) {
            return;
        }
        // Forward-truncate: anything after the cursor is no longer reachable
        // by "forward" once the user takes a new path.
        if !self.entries.is_empty() && self.position + 1 < self.entries.len() {
            self.entries.truncate(self.position + 1);
        }
        self.entries.push(id);
        self.position = self.entries.len() - 1;

        // Enforce cap — drop from the front, adjusting the cursor.
        while self.entries.len() > HISTORY_CAP {
            self.entries.remove(0);
            self.position = self.position.saturating_sub(1);
        }
    }

    /// Step the cursor one position back and return the now-current id.
    /// `None` if already at the head (or empty).
    pub fn back(&mut self) -> Option<&str> {
        if !self.can_go_back() {
            return None;
        }
        self.position -= 1;
        self.current()
    }

    /// Step the cursor one position forward and return the now-current id.
    pub fn forward(&mut self) -> Option<&str> {
        if !self.can_go_forward() {
            return None;
        }
        self.position += 1;
        self.current()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_history_has_nowhere_to_go() {
        let mut h = History::new();
        assert!(h.current().is_none());
        assert!(!h.can_go_back());
        assert!(!h.can_go_forward());
        // Regression: original `navigate_forward` did `len() - 1` and
        // underflowed on an empty Vec. These must not panic.
        assert!(h.back().is_none());
        assert!(h.forward().is_none());
    }

    #[test]
    fn push_appends_and_moves_cursor() {
        let mut h = History::new();
        h.push("a".into());
        h.push("b".into());
        h.push("c".into());
        assert_eq!(h.current(), Some("c"));
        assert!(h.can_go_back());
        assert!(!h.can_go_forward());
    }

    #[test]
    fn consecutive_duplicates_are_skipped() {
        let mut h = History::new();
        h.push("a".into());
        h.push("a".into());
        h.push("a".into());
        assert_eq!(h.current(), Some("a"));
        assert!(!h.can_go_back());
    }

    #[test]
    fn push_after_back_truncates_forward_history() {
        let mut h = History::new();
        h.push("a".into());
        h.push("b".into());
        h.push("c".into());
        h.back(); // now at b
        h.push("d".into()); // c should be gone
        assert_eq!(h.current(), Some("d"));
        assert!(!h.can_go_forward());
    }

    #[test]
    fn back_and_forward_round_trip() {
        let mut h = History::new();
        h.push("a".into());
        h.push("b".into());
        h.push("c".into());
        assert_eq!(h.back(), Some("b"));
        assert_eq!(h.back(), Some("a"));
        assert_eq!(h.back(), None); // at head
        assert_eq!(h.forward(), Some("b"));
        assert_eq!(h.forward(), Some("c"));
        assert_eq!(h.forward(), None); // at tail
    }

    #[test]
    fn reset_replaces_history() {
        let mut h = History::new();
        h.push("a".into());
        h.push("b".into());
        h.reset("fresh".into());
        assert_eq!(h.current(), Some("fresh"));
        assert!(!h.can_go_back());
        assert!(!h.can_go_forward());
    }

    #[test]
    fn cap_enforced_by_dropping_from_front() {
        let mut h = History::new();
        for i in 0..(HISTORY_CAP + 50) {
            h.push(format!("note-{}", i));
        }
        assert_eq!(
            h.current(),
            Some(format!("note-{}", HISTORY_CAP + 49).as_str())
        );
        // Can still navigate back through at most HISTORY_CAP entries.
        let mut back_count = 0;
        while h.back().is_some() {
            back_count += 1;
        }
        assert_eq!(back_count, HISTORY_CAP - 1);
    }

    #[test]
    fn recent_returns_distinct_most_recent_first_excluding_current() {
        let mut h = History::new();
        h.push("a".into());
        h.push("b".into());
        h.push("c".into());
        h.push("a".into()); // revisit
        h.push("d".into());
        // Cursor is at d. Recent should be [a, c, b] — distinct, newest first,
        // excluding d (current). 'a' appears once even though it was visited twice.
        let r = h.recent(5);
        assert_eq!(r, vec!["a".to_string(), "c".to_string(), "b".to_string()]);
    }

    #[test]
    fn recent_respects_cap() {
        let mut h = History::new();
        for i in 0..10 {
            h.push(format!("n{}", i));
        }
        assert_eq!(h.recent(3).len(), 3);
    }

    #[test]
    fn cap_preserves_cursor_near_tail() {
        // Push to exactly cap, then one more; the new entry should be
        // current and reachable.
        let mut h = History::new();
        for i in 0..HISTORY_CAP {
            h.push(format!("n{}", i));
        }
        h.push("new".into());
        assert_eq!(h.current(), Some("new"));
    }
}
