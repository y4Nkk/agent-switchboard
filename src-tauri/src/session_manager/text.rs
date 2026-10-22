pub(super) fn compact_chars(text: &str) -> impl Iterator<Item = char> + '_ {
    text.split_whitespace().enumerate().flat_map(|(index, word)| {
        (index > 0).then_some(' ').into_iter().chain(word.chars())
    })
}

pub(super) fn raw_offset(text: &str, compact_offset: usize) -> Option<usize> {
    let mut compact = 0;
    let mut started = false;
    let mut gap = None;
    for (raw, character) in text.chars().enumerate() {
        if character.is_whitespace() {
            if started && gap.is_none() { gap = Some(raw); }
            continue;
        }
        if let Some(space) = gap.take() {
            if compact == compact_offset { return Some(space); }
            compact += 1;
        }
        if compact == compact_offset { return Some(raw); }
        compact += 1;
        started = true;
    }
    None
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
        let mut matched = 0;
        let mut positions = std::collections::VecDeque::with_capacity(self.pattern.len());
        for (position, character) in compact_chars(text).enumerate() {
            for lower in character.to_lowercase().map(fold_sigma) {
                if positions.len() == self.pattern.len() { positions.pop_front(); }
                positions.push_back(position);
                while matched > 0 && lower != self.pattern[matched] {
                    matched = self.prefix[matched - 1];
                }
                if lower == self.pattern[matched] { matched += 1; }
                if matched == self.pattern.len() { return positions.front().copied(); }
            }
        }
        None
    }

    pub(super) fn excerpt(&self, text: &str) -> String {
        let start = self.find(text).unwrap_or(0).saturating_sub(70);
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
    fn maps_compact_match_positions_back_to_original_characters() {
        let text = "  😀\n\t  İ目标   tail ";
        for (compact, raw) in [(0, 2), (1, 3), (2, 7), (3, 8), (4, 9), (5, 10), (6, 13)] {
            assert_eq!(raw_offset(text, compact), Some(raw));
        }
        let found = TextQuery::new("İ目标").find(text).unwrap();
        assert_eq!(text.chars().skip(raw_offset(text, found).unwrap()).take(3).collect::<String>(), "İ目标");
        assert_eq!(raw_offset("  \n ", 0), None);
        assert_eq!(raw_offset("x  ", 1), None);
    }
}
