//! Candidate list management
//!
//! Handles the list of conversion candidates with pagination support.

/// Source of a conversion candidate — which subsystem produced it.
/// Presentation ([`label`](Self::label), [`is_deletable`](Self::is_deletable))
/// is derived from this on read, never stored, so it can't fall out of sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateSource {
    /// User dictionary lookup
    UserDictionary,
    /// Learning cache (user history)
    Learning,
    /// Model inference result
    Model,
    /// System dictionary lookup (also covers reading→symbol lookups via
    /// mozc's symbol.tsv — they're treated as just another dictionary).
    Dictionary,
    /// Rewriter-generated variant (half-width katakana, symbol)
    Rewriter,
    /// Date/time candidate rendered from the clock (`[date]` phrases)
    Date,
    /// Hiragana/katakana fallback
    Fallback,
}

impl CandidateSource {
    /// Aux-text label telling the user which subsystem produced the
    /// candidate. Empty for sources that aren't worth calling out (Fallback).
    pub fn label(&self) -> &'static str {
        match self {
            CandidateSource::UserDictionary => "\u{1F464} \u{30E6}\u{30FC}\u{30B6}\u{30FC}", // 👤 ユーザー
            CandidateSource::Learning => "\u{1F4DD} \u{5B66}\u{7FD2}", // 📝 学習
            CandidateSource::Model => "\u{1F916} AI",                  // 🤖 AI
            CandidateSource::Dictionary => "\u{1F4DA} \u{8F9E}\u{66F8}", // 📚 辞書
            CandidateSource::Rewriter => "\u{1F504} \u{5909}\u{63DB}", // 🔄 変換
            CandidateSource::Date => "\u{1F4C5} \u{65E5}\u{4ED8}",     // 📅 日付
            CandidateSource::Fallback => "",
        }
    }

    /// Short glyph for tight spots like the Ctrl+R filter header in the
    /// conversion aux text. Empty for Fallback, which has no view of its
    /// own — its kana ride at the tail of the rewriter view.
    pub fn emoji(&self) -> &'static str {
        match self {
            CandidateSource::UserDictionary => "\u{1F464}", // 👤
            CandidateSource::Learning => "\u{1F4DD}",       // 📝
            CandidateSource::Model => "\u{1F916}",          // 🤖
            CandidateSource::Dictionary => "\u{1F4DA}",     // 📚
            CandidateSource::Rewriter => "\u{1F504}",       // 🔄
            CandidateSource::Date => "\u{1F4C5}",           // 📅
            CandidateSource::Fallback => "",
        }
    }

    /// Whether candidates from this source can be removed from the learning
    /// history with Ctrl+Backspace / Ctrl+Delete.
    pub fn is_deletable(&self) -> bool {
        matches!(self, CandidateSource::Learning)
    }

    /// Whether committing a candidate from this source is recorded in the
    /// learning cache. Date/time candidates are rendered from the clock, so
    /// a recorded one would resurface later as a stale date.
    pub fn is_learnable(&self) -> bool {
        !matches!(self, CandidateSource::Date)
    }
}

/// A single conversion candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The converted text
    pub text: String,
    /// The original reading (hiragana)
    pub reading: Option<String>,
    /// Which subsystem produced this candidate, if known.
    pub source: Option<CandidateSource>,
    /// Per-candidate description shown as the right-side comment on the
    /// candidate (mozc-style). Only set when the candidate itself has a
    /// meaningful description — symbol descriptions like `三点リーダ`,
    /// rewriter descriptions like `[全]英大文字`. Source labels are
    /// intentionally excluded so they don't duplicate the aux text.
    pub description: Option<String>,
}

impl Candidate {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            reading: None,
            source: None,
            description: None,
        }
    }

    pub fn with_reading(text: impl Into<String>, reading: impl Into<String>) -> Self {
        Self {
            reading: Some(reading.into()),
            ..Self::new(text)
        }
    }

    /// Aux-text source label (`🤖 AI`, `📚 辞書`, ...) derived from `source`;
    /// `None` when the source is unknown or has no label.
    pub fn source_label(&self) -> Option<&'static str> {
        self.source.map(|s| s.label()).filter(|l| !l.is_empty())
    }

    /// Whether this candidate can be removed from the learning history with
    /// Ctrl+Backspace / Ctrl+Delete. See [`CandidateSource::is_deletable`].
    pub fn is_deletable(&self) -> bool {
        self.source.is_some_and(|s| s.is_deletable())
    }
}

/// Upper kana quick-select panel shown above the composing line when
/// pressing Up from the top of the conversion list (top-to-bottom:
/// Katakana, Hiragana).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpperCandidates {
    pub candidates: Vec<Candidate>,
    pub cursor: usize,
}

/// A list of candidates with pagination and selection support
#[derive(Debug, Clone)]
pub struct CandidateList {
    /// All candidates
    candidates: Vec<Candidate>,
    /// Currently selected candidate index
    cursor: usize,
    /// Number of candidates per page
    page_size: usize,
    /// Optional upper panel candidates (Katakana / Hiragana) shown above the composing line
    upper: Option<UpperCandidates>,
}

impl CandidateList {
    /// Default page size for candidate display
    pub const DEFAULT_PAGE_SIZE: usize = 9;

    /// Create a new candidate list
    pub fn new(candidates: Vec<Candidate>) -> Self {
        Self {
            candidates,
            cursor: 0,
            page_size: Self::DEFAULT_PAGE_SIZE,
            upper: None,
        }
    }

    /// Create a candidate list from strings (test fixture).
    #[cfg(test)]
    pub fn from_strings(strings: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self::new(strings.into_iter().map(Candidate::new).collect())
    }

    /// Get all candidates
    pub fn candidates(&self) -> &[Candidate] {
        &self.candidates
    }

    /// Get the number of candidates
    pub fn len(&self) -> usize {
        self.candidates.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }

    /// Get the current cursor position
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Get the page size
    pub fn page_size(&self) -> usize {
        self.page_size
    }

    /// Get the current page number (0-indexed)
    pub fn current_page(&self) -> usize {
        self.cursor.checked_div(self.page_size).unwrap_or(0)
    }

    /// Get the total number of pages
    pub fn total_pages(&self) -> usize {
        if self.page_size == 0 || self.candidates.is_empty() {
            0
        } else {
            self.candidates.len().div_ceil(self.page_size)
        }
    }

    /// Get the start index of the current page
    pub fn page_start(&self) -> usize {
        self.current_page() * self.page_size
    }

    /// Get the candidates for the current page
    pub fn page_candidates(&self) -> &[Candidate] {
        let start = self.page_start();
        let end = (start + self.page_size).min(self.candidates.len());
        &self.candidates[start..end]
    }

    /// Get the cursor position within the current page (0-indexed)
    pub fn page_cursor(&self) -> usize {
        self.cursor - self.page_start()
    }

    /// Get the upper panel state if active
    pub fn upper(&self) -> Option<&UpperCandidates> {
        self.upper.as_ref()
    }

    /// Whether focus is currently inside the upper panel
    pub fn has_upper_focus(&self) -> bool {
        self.upper.is_some()
    }

    /// Active cursor within the current page of normal candidates, or `None`
    /// while focus is inside the upper panel.
    pub fn active_page_cursor(&self) -> Option<usize> {
        if self.upper.is_some() {
            None
        } else {
            Some(self.page_cursor())
        }
    }

    /// Open the upper panel with the given candidates (top-to-bottom order),
    /// selecting the bottom-most item (closest to the composing line).
    pub fn open_upper(&mut self, candidates: Vec<Candidate>) -> bool {
        if candidates.is_empty() {
            return false;
        }
        let cursor = candidates.len() - 1;
        self.upper = Some(UpperCandidates { candidates, cursor });
        true
    }

    /// Close the upper panel if open
    pub fn close_upper(&mut self) {
        self.upper = None;
    }

    /// Get the currently selected candidate
    pub fn selected(&self) -> Option<&Candidate> {
        if let Some(upper) = &self.upper {
            upper.candidates.get(upper.cursor)
        } else {
            self.candidates.get(self.cursor)
        }
    }

    /// Get the currently selected text
    pub fn selected_text(&self) -> Option<&str> {
        self.selected().map(|c| c.text.as_str())
    }

    /// Move to the next candidate
    pub fn move_next(&mut self) -> bool {
        if let Some(upper) = &mut self.upper {
            if upper.cursor + 1 < upper.candidates.len() {
                upper.cursor += 1;
            } else {
                // Moving down past the bottom of the upper panel closes it
                // and returns focus to the first normal candidate.
                self.upper = None;
                self.cursor = 0;
            }
            return true;
        }
        if self.cursor + 1 < self.candidates.len() {
            self.cursor += 1;
            true
        } else if !self.candidates.is_empty() {
            // Wrap to beginning
            self.cursor = 0;
            true
        } else {
            false
        }
    }

    /// Move to the previous candidate
    pub fn move_prev(&mut self) -> bool {
        if let Some(upper) = &mut self.upper {
            if upper.cursor > 0 {
                upper.cursor -= 1;
            }
            return true;
        }
        if self.cursor > 0 {
            self.cursor -= 1;
            true
        } else if !self.candidates.is_empty() {
            // Wrap to end
            self.cursor = self.candidates.len() - 1;
            true
        } else {
            false
        }
    }

    /// Move to the next page
    pub fn next_page(&mut self) -> bool {
        if self.candidates.is_empty() {
            return false;
        }
        self.upper = None;

        let next_page_start = self.page_start() + self.page_size;
        if next_page_start < self.candidates.len() {
            self.cursor = next_page_start;
            true
        } else {
            // Wrap to first page
            self.cursor = 0;
            true
        }
    }

    /// Move to the previous page
    pub fn prev_page(&mut self) -> bool {
        if self.candidates.is_empty() {
            return false;
        }
        self.upper = None;

        let current_page = self.current_page();
        if current_page > 0 {
            self.cursor = (current_page - 1) * self.page_size;
            true
        } else {
            // Wrap to last page
            let last_page = self.total_pages().saturating_sub(1);
            self.cursor = last_page * self.page_size;
            true
        }
    }

    /// Select a candidate by index within the current page (1-9)
    pub fn select_on_page(&mut self, page_index: usize) -> Option<&Candidate> {
        if page_index == 0 || page_index > self.page_size {
            return None;
        }

        let absolute_index = self.page_start() + page_index - 1;
        if absolute_index < self.candidates.len() {
            self.upper = None;
            self.cursor = absolute_index;
            self.selected()
        } else {
            None
        }
    }

    /// Move the cursor to `cursor`, clamped into the list (0 when empty).
    pub fn set_cursor(&mut self, cursor: usize) {
        self.upper = None;
        self.cursor = cursor.min(self.candidates.len().saturating_sub(1));
    }
}

impl Default for CandidateList {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_candidate_list_basic() {
        let candidates = CandidateList::from_strings(["今日", "京", "恭"]);
        assert_eq!(candidates.len(), 3);
        assert_eq!(candidates.selected_text(), Some("今日"));
    }

    #[test]
    fn test_candidate_list_navigation() {
        let mut candidates = CandidateList::from_strings(["a", "b", "c"]);

        assert!(candidates.move_next());
        assert_eq!(candidates.selected_text(), Some("b"));

        assert!(candidates.move_next());
        assert_eq!(candidates.selected_text(), Some("c"));

        // Wrap around
        assert!(candidates.move_next());
        assert_eq!(candidates.selected_text(), Some("a"));

        // Wrap back
        assert!(candidates.move_prev());
        assert_eq!(candidates.selected_text(), Some("c"));
    }

    #[test]
    fn test_candidate_list_pagination() {
        // Default page_size is 9, so 20 items = 3 pages (9+9+2)
        let items: Vec<_> = (1..=20).map(|i| format!("item{}", i)).collect();
        let mut candidates = CandidateList::from_strings(items);

        assert_eq!(candidates.total_pages(), 3);
        assert_eq!(candidates.current_page(), 0);
        assert_eq!(candidates.page_candidates().len(), 9);

        candidates.next_page();
        assert_eq!(candidates.current_page(), 1);
        assert_eq!(candidates.page_start(), 9);

        candidates.next_page();
        assert_eq!(candidates.current_page(), 2);
        assert_eq!(candidates.page_candidates().len(), 2);

        // Wrap to first page
        candidates.next_page();
        assert_eq!(candidates.current_page(), 0);
    }

    #[test]
    fn test_candidate_list_select_on_page() {
        let items: Vec<_> = (1..=20).map(|i| format!("item{}", i)).collect();
        let mut candidates = CandidateList::from_strings(items);

        // Select item 3 on first page
        candidates.select_on_page(3);
        assert_eq!(candidates.selected_text(), Some("item3"));

        // Move to second page and select item 2
        candidates.next_page();
        candidates.select_on_page(2);
        assert_eq!(candidates.selected_text(), Some("item11")); // 9 + 2 = 11
    }

    #[test]
    fn test_upper_candidates_navigation_and_auto_close() {
        let mut candidates = CandidateList::from_strings(["今日", "京"]);

        // open_upper selects the bottom-most item (closest to the typing line)
        assert!(candidates.open_upper(vec![Candidate::new("キョウ"), Candidate::new("きょう"),]));
        assert!(candidates.has_upper_focus());
        assert_eq!(candidates.active_page_cursor(), None);
        assert_eq!(candidates.selected_text(), Some("きょう"));
        assert_eq!(candidates.upper().unwrap().cursor, 1);

        // Moving up steps to katakana and clamps at the top
        assert!(candidates.move_prev());
        assert_eq!(candidates.selected_text(), Some("キョウ"));
        assert_eq!(candidates.upper().unwrap().cursor, 0);

        assert!(candidates.move_prev());
        assert_eq!(candidates.selected_text(), Some("キョウ"));
        assert_eq!(candidates.upper().unwrap().cursor, 0);

        // Moving down returns to hiragana, then closes upper focus onto normal index 0
        assert!(candidates.move_next());
        assert_eq!(candidates.selected_text(), Some("きょう"));
        assert!(candidates.has_upper_focus());

        assert!(candidates.move_next());
        assert!(!candidates.has_upper_focus());
        assert_eq!(candidates.active_page_cursor(), Some(0));
        assert_eq!(candidates.selected_text(), Some("今日"));
    }

    #[test]
    fn test_upper_closed_on_page_or_digit_select() {
        let items: Vec<_> = (1..=12).map(|i| format!("item{}", i)).collect();
        let mut candidates = CandidateList::from_strings(items);

        assert!(
            candidates.open_upper(vec![Candidate::new("カタカナ"), Candidate::new("ひらがな"),])
        );
        assert_eq!(
            candidates.select_on_page(2).map(|c| c.text.as_str()),
            Some("item2")
        );
        assert!(!candidates.has_upper_focus());
        assert_eq!(candidates.active_page_cursor(), Some(1));

        assert!(
            candidates.open_upper(vec![Candidate::new("カタカナ"), Candidate::new("ひらがな"),])
        );
        assert!(candidates.next_page());
        assert!(!candidates.has_upper_focus());
        assert_eq!(candidates.current_page(), 1);

        assert!(
            candidates.open_upper(vec![Candidate::new("カタカナ"), Candidate::new("ひらがな"),])
        );
        candidates.set_cursor(0);
        assert!(!candidates.has_upper_focus());
        assert_eq!(candidates.active_page_cursor(), Some(0));
    }
}
