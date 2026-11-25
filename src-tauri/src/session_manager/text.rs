use std::collections::VecDeque;
use std::ops::{ControlFlow, Range};

pub(super) fn compact_chars(text: &str) -> impl Iterator<Item = char> + '_ {
    text.split_whitespace().enumerate().flat_map(|(index, word)| {
        (index > 0).then_some(' ').into_iter().chain(word.chars())
    })
}

pub(super) fn validate_query(text: &str) -> Result<(), String> {
    if text.chars().count() > 4000 { return Err("搜索内容最多 4000 个字符".into()); }
    Ok(())
}

pub(super) struct TextMatch {
    pub compact_offset: usize,
    pub range: Range<usize>,
}

pub(super) struct TextQuery {
    pattern: Vec<char>,
    prefix: Vec<usize>,
}

impl TextQuery {
    pub(super) fn new(text: &str) -> Self {
        let pattern: Vec<_> = compact_chars(text).flat_map(char::to_lowercase).map(fold_sigma).collect();
        let mut prefix = vec![0; pattern.len()];
        for index in 1..pattern.len() {
            let mut matched = prefix[index - 1];
            while matched > 0 && pattern[index] != pattern[matched] {
                matched = prefix[matched - 1];
            }
            if pattern[index] == pattern[matched] { matched += 1; }
            prefix[index] = matched;
        }
        Self { pattern, prefix }
    }

    pub(super) fn is_empty(&self) -> bool { self.pattern.is_empty() }

    pub(super) fn identity(&self) -> String { self.pattern.iter().collect() }

    pub(super) fn find(&self, text: &str) -> Option<usize> {
        if self.is_empty() { return Some(0); }
        self.first_match(text, &|| Ok(())).ok().flatten().map(|found| found.compact_offset)
    }

    pub(super) fn first_match(&self, text: &str, checkpoint: &dyn Fn() -> Result<(), String>) -> Result<Option<TextMatch>, String> {
        self.scan_matches(text, checkpoint, ControlFlow::Break)
    }

    pub(super) fn scan_matches(&self, text: &str, checkpoint: &dyn Fn() -> Result<(), String>,
        mut visit: impl FnMut(TextMatch) -> ControlFlow<TextMatch>,
    ) -> Result<Option<TextMatch>, String> {
        if self.is_empty() { return Ok(None); }
        let mut matched = 0;
        let mut positions = VecDeque::with_capacity(self.pattern.len());
        let mut compact = 0;
        let mut previous_space = true;
        for (raw, original) in text.chars().enumerate() {
            if raw % 4096 == 0 { checkpoint()?; }
            let space = original.is_whitespace();
            if space && previous_space { continue; }
            previous_space = space;
            let character = if space { ' ' } else { original };
            for lower in character.to_lowercase().map(fold_sigma) {
                if positions.len() == self.pattern.len() { positions.pop_front(); }
                positions.push_back((compact, raw));
                while matched > 0 && lower != self.pattern[matched] {
                    matched = self.prefix[matched - 1];
                }
                if lower == self.pattern[matched] { matched += 1; }
                if matched == self.pattern.len() {
                    let (compact_offset, start) = positions[0];
                    if let ControlFlow::Break(found) = visit(TextMatch { compact_offset, range: start..raw + 1 }) {
                        return Ok(Some(found));
                    }
                    matched = 0;
                    break;
                }
            }
            compact += 1;
        }
        Ok(None)
    }

    pub(super) fn excerpt(&self, text: &str) -> String {
        self.excerpt_at(text, self.find(text).unwrap_or(0))
    }

    pub(super) fn excerpt_at(&self, text: &str, position: usize) -> String {
        let start = position.saturating_sub(70);
        let length = self.pattern.len().saturating_add(140).max(220);
        let mut characters = compact_chars(text).skip(start);
        let excerpt: String = characters.by_ref().take(length).collect();
        format!("{}{}{}", if start > 0 { "…" } else { "" }, excerpt,
            if characters.next().is_some() { "…" } else { "" })
    }
}

fn fold_sigma(character: char) -> char { if character == 'ς' { 'σ' } else { character } }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_unicode_case_and_whitespace_without_normalizing_the_whole_message() {
        assert!(TextQuery::new("ÄPFEL  连接\nERROR42").find("prefix äpfel\n连接\tError42 suffix").is_some());
        assert!(TextQuery::new("ΟΣ").find("ος").is_some());
        assert!(TextQuery::new("İ").find("i\u{307}").is_some());
        let text = format!("{} target message tail", "ab".repeat(100_000));
        let query = TextQuery::new("target message");
        let excerpt = query.excerpt(&text);
        assert!(excerpt.contains("target message"));
        assert!(excerpt.starts_with('…'));
        assert!(excerpt.chars().count() <= 222);
    }

    #[test]
    fn visits_each_non_overlapping_match_with_original_character_ranges() {
        let text = "  😀\n\t  İ目标   tail ";
        let found = TextQuery::new("İ目标 tail").first_match(text, &|| Ok(())).unwrap().unwrap();
        assert_eq!(found.compact_offset, 2);
        assert_eq!(found.range, 7..17);
        let mut ranges = Vec::new();
        let _ = TextQuery::new("aa").scan_matches("😀AaA aa", &|| Ok(()), |found| {
            ranges.push(found.range);
            ControlFlow::Continue(())
        }).unwrap();
        assert_eq!(ranges, [1..3, 5..7]);
    }
}
