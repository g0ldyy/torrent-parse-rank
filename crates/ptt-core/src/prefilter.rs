use std::collections::HashMap;

use aho_corasick::{AhoCorasick, AhoCorasickBuilder};
use regex_syntax::hir::{Class, Hir, HirKind};

const MAX_GROUPS: usize = 4;
const MAX_ALTERNATIVES: usize = 64;
const MAX_CLASS_SIZE: usize = 14;
const MAX_HIT_WORDS: usize = 32;
const MIN_LITERAL_LEN: usize = 2;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Gate {
    groups: [u16; MAX_GROUPS],
    len: u8,
    unicode_sensitive: u8,
}

impl Gate {
    #[inline]
    pub(crate) fn hit(self, hits: &Hits) -> bool {
        self.groups[..usize::from(self.len)]
            .iter()
            .enumerate()
            .all(|(index, &bit)| {
                hits.non_ascii && self.unicode_sensitive & (1 << index) != 0
                    || hits.bits[usize::from(bit >> 6)] & (1 << (bit & 63)) != 0
            })
    }

    #[cfg(test)]
    pub(crate) fn is_active(self) -> bool {
        self.len != 0
    }
}

pub(crate) struct Hits {
    bits: [u64; MAX_HIT_WORDS],
    non_ascii: bool,
}

impl Default for Hits {
    fn default() -> Self {
        Self {
            bits: [0; MAX_HIT_WORDS],
            non_ascii: false,
        }
    }
}

pub(crate) struct Prefilter {
    matcher: AhoCorasick,
    literal_groups: Vec<Box<[u16]>>,
    words: usize,
}

impl Prefilter {
    pub(crate) fn build(specs: &[Vec<Vec<String>>]) -> (Self, Vec<Gate>) {
        let mut literals = Vec::<String>::new();
        let mut literal_ids = HashMap::<String, usize>::new();
        let mut literal_groups = Vec::<Vec<u16>>::new();
        let mut gates = Vec::with_capacity(specs.len());
        let mut next_group = 0usize;

        for spec in specs {
            let mut gate = Gate::default();
            for group in spec.iter().take(MAX_GROUPS) {
                assert!(next_group < MAX_HIT_WORDS * 64, "too many prefilter groups");
                let bit = u16::try_from(next_group).expect("prefilter group fits in u16");
                gate.groups[usize::from(gate.len)] = bit;
                // PCRE2's Unicode simple-fold orbits contain non-ASCII
                // equivalents only for ASCII k (K) and s (ſ).
                if group.iter().any(|literal| {
                    literal
                        .bytes()
                        .any(|byte| matches!(byte.to_ascii_lowercase(), b'k' | b's'))
                }) {
                    gate.unicode_sensitive |= 1 << gate.len;
                }
                gate.len += 1;
                next_group += 1;

                for literal in group {
                    let folded = literal.to_ascii_lowercase();
                    let id = if let Some(&id) = literal_ids.get(&folded) {
                        id
                    } else {
                        let id = literals.len();
                        literal_ids.insert(folded.clone(), id);
                        literals.push(folded);
                        literal_groups.push(Vec::new());
                        id
                    };
                    if literal_groups[id].last().copied() != Some(bit) {
                        literal_groups[id].push(bit);
                    }
                }
            }
            gates.push(gate);
        }

        let matcher = AhoCorasickBuilder::new()
            .ascii_case_insensitive(true)
            .build(&literals)
            .expect("valid prefilter literals");
        (
            Self {
                matcher,
                literal_groups: literal_groups
                    .into_iter()
                    .map(Vec::into_boxed_slice)
                    .collect(),
                words: next_group.div_ceil(64),
            },
            gates,
        )
    }

    #[inline]
    pub(crate) fn scan(&self, title: &str, hits: &mut Hits) {
        hits.bits[..self.words].fill(0);
        hits.non_ascii = !title.is_ascii();
        for found in self.matcher.find_overlapping_iter(title) {
            for &group in &self.literal_groups[found.pattern().as_usize()] {
                hits.bits[usize::from(group >> 6)] |= 1 << (group & 63);
            }
        }
    }
}

pub(crate) fn derive(pattern: &str) -> Vec<Vec<String>> {
    let (base, marked, positive_assertions) = strip_assertions_and_backrefs(pattern);
    let marked_hir = regex_syntax::Parser::new().parse(&marked).ok();
    let mut groups = derive_from_pattern(&base);
    for (marker, assertion) in positive_assertions {
        if marked_hir
            .as_ref()
            .is_some_and(|hir| requires_literal(hir, marker.as_bytes()))
        {
            groups.extend(derive(&assertion));
        }
    }
    groups.sort_by(|left, right| group_strength(left, right));
    groups.dedup();
    groups.truncate(MAX_GROUPS);
    groups
}

fn derive_from_pattern(pattern: &str) -> Vec<Vec<String>> {
    let Ok(hir) = regex_syntax::Parser::new().parse(pattern) else {
        return Vec::new();
    };
    let mut groups: Vec<Vec<String>> = match hir.kind() {
        HirKind::Concat(parts) => parts.iter().filter_map(required_literals).collect(),
        _ => required_literals(&hir).into_iter().collect(),
    };
    groups.retain(|group| usable_group(group));
    groups
}

fn required_literals(hir: &Hir) -> Option<Vec<String>> {
    match hir.kind() {
        HirKind::Literal(literal) => {
            let text = std::str::from_utf8(&literal.0).ok()?;
            text.is_ascii().then(|| vec![text.to_owned()])
        }
        HirKind::Class(class) => expand_class(class),
        HirKind::Capture(capture) => required_literals(&capture.sub),
        HirKind::Repetition(repetition) if repetition.min > 0 => required_literals(&repetition.sub),
        HirKind::Concat(parts) => parts
            .iter()
            .filter_map(required_literals)
            .filter(|group| usable_group(group))
            .min_by(|left, right| group_strength(left, right)),
        HirKind::Alternation(branches) => {
            let mut alternatives = Vec::new();
            for branch in branches {
                alternatives.extend(required_literals(branch)?);
                if alternatives.len() > MAX_ALTERNATIVES {
                    return None;
                }
            }
            alternatives.sort_unstable();
            alternatives.dedup();
            Some(alternatives)
        }
        _ => None,
    }
}

fn expand_class(class: &Class) -> Option<Vec<String>> {
    let mut values = Vec::new();
    match class {
        Class::Unicode(class) => {
            for range in class.iter() {
                if !range.start().is_ascii() || !range.end().is_ascii() {
                    return None;
                }
                for value in u32::from(range.start())..=u32::from(range.end()) {
                    values.push(char::from_u32(value)?.to_string());
                    if values.len() > MAX_CLASS_SIZE {
                        return None;
                    }
                }
            }
        }
        Class::Bytes(class) => {
            for range in class.iter() {
                if !range.start().is_ascii() || !range.end().is_ascii() {
                    return None;
                }
                for value in range.start()..=range.end() {
                    values.push(char::from(value).to_string());
                    if values.len() > MAX_CLASS_SIZE {
                        return None;
                    }
                }
            }
        }
    }
    Some(values)
}

fn usable_group(group: &[String]) -> bool {
    !group.is_empty()
        && group.len() <= MAX_ALTERNATIVES
        && group
            .iter()
            .all(|literal| literal.is_ascii() && literal.len() >= MIN_LITERAL_LEN)
}

fn group_strength(left: &[String], right: &[String]) -> std::cmp::Ordering {
    let left_min = left.iter().map(String::len).min().unwrap_or_default();
    let right_min = right.iter().map(String::len).min().unwrap_or_default();
    right_min
        .cmp(&left_min)
        .then_with(|| left.len().cmp(&right.len()))
        .then_with(|| left.cmp(right))
}

fn requires_literal(hir: &Hir, needle: &[u8]) -> bool {
    match hir.kind() {
        HirKind::Literal(literal) => literal.0.windows(needle.len()).any(|part| part == needle),
        HirKind::Capture(capture) => requires_literal(&capture.sub, needle),
        HirKind::Repetition(repetition) => {
            repetition.min > 0 && requires_literal(&repetition.sub, needle)
        }
        HirKind::Concat(parts) => parts.iter().any(|part| requires_literal(part, needle)),
        HirKind::Alternation(branches) => branches
            .iter()
            .all(|branch| requires_literal(branch, needle)),
        _ => false,
    }
}

fn strip_assertions_and_backrefs(pattern: &str) -> (String, String, Vec<(String, String)>) {
    let bytes = pattern.as_bytes();
    let mut base = String::with_capacity(pattern.len());
    let mut marked = String::with_capacity(pattern.len());
    let mut positives = Vec::new();
    let mut index = 0usize;
    let mut in_class = false;

    while index < bytes.len() {
        if bytes[index] == b'\\' {
            if !in_class && bytes.get(index + 1).is_some_and(u8::is_ascii_digit) {
                index += 2;
                while bytes.get(index).is_some_and(u8::is_ascii_digit) {
                    index += 1;
                }
                continue;
            }
            let end = (index + 2).min(bytes.len());
            base.push_str(&pattern[index..end]);
            marked.push_str(&pattern[index..end]);
            index = end;
            continue;
        }
        if bytes[index] == b'[' {
            in_class = true;
        } else if bytes[index] == b']' {
            in_class = false;
        }
        if !in_class && bytes[index] == b'(' {
            let (prefix_len, positive) = if pattern[index..].starts_with("(?<=") {
                (4, true)
            } else if pattern[index..].starts_with("(?<!") {
                (4, false)
            } else if pattern[index..].starts_with("(?=") {
                (3, true)
            } else if pattern[index..].starts_with("(?!") {
                (3, false)
            } else {
                (0, false)
            };
            if prefix_len != 0
                && let Some(end) = assertion_end(pattern, index + prefix_len)
            {
                if positive {
                    let marker = format!("ZQASSERT{}QZ", positives.len());
                    marked.push_str(&marker);
                    positives.push((marker, pattern[index + prefix_len..end].to_owned()));
                }
                index = end + 1;
                continue;
            }
        }
        let character = pattern[index..]
            .chars()
            .next()
            .expect("index is within pattern");
        base.push(character);
        marked.push(character);
        index += character.len_utf8();
    }
    (base, marked, positives)
}

fn assertion_end(pattern: &str, body_start: usize) -> Option<usize> {
    let bytes = pattern.as_bytes();
    let mut depth = 1usize;
    let mut index = body_start;
    let mut in_class = false;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index = (index + 2).min(bytes.len());
            continue;
        }
        match bytes[index] {
            b'[' => in_class = true,
            b']' => in_class = false,
            b'(' if !in_class => depth += 1,
            b')' if !in_class => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_conservative_groups() {
        assert_eq!(
            derive(r"\bBlu[ .-]*Ray\b(?=.*remux)"),
            vec![
                vec!["remux".to_owned()],
                vec!["Blu".to_owned()],
                vec!["Ray".to_owned()]
            ]
        );
        assert_eq!(
            derive(r"\b(?:WEBRip|BluRay)\b"),
            vec![vec!["BluRay".to_owned(), "WEBRip".to_owned()]]
        );
        assert_eq!(
            derive(r"(?:8|10|12)[.-]?(?=bit\b)"),
            vec![vec!["bit".to_owned()]]
        );
        assert!(
            derive(r"foo|bar(?=baz)")
                .iter()
                .all(|group| group.iter().all(|literal| literal != "baz"))
        );
        assert!(derive(r"\d{1,4}").is_empty());
    }

    #[test]
    fn gate_requires_every_group() {
        let specs = vec![vec![vec!["bluray".to_owned()], vec!["remux".to_owned()]]];
        let (prefilter, gates) = Prefilter::build(&specs);
        let mut hits = Hits::default();
        prefilter.scan("Movie.2024.BluRay.x265", &mut hits);
        assert!(!gates[0].hit(&hits));
        prefilter.scan("Movie.2024.BluRay.REMUX", &mut hits);
        assert!(gates[0].hit(&hits));
    }

    #[test]
    fn unicode_titles_still_use_safe_groups() {
        let specs = vec![vec![vec!["bluray".to_owned()]]];
        let (prefilter, gates) = Prefilter::build(&specs);
        let mut hits = Hits::default();
        prefilter.scan("Мстители WEB-DL", &mut hits);
        assert!(!gates[0].hit(&hits));
        prefilter.scan("Мстители BluRay", &mut hits);
        assert!(gates[0].hit(&hits));
    }

    #[test]
    fn unicode_simple_folds_cannot_cause_false_negatives() {
        let specs = vec![
            vec![vec!["remastered".to_owned()]],
            vec![vec!["bluray".to_owned()]],
        ];
        let (prefilter, gates) = Prefilter::build(&specs);
        let mut hits = Hits::default();
        prefilter.scan("Movie remaſtered", &mut hits);
        assert!(gates[0].hit(&hits));
        assert!(!gates[1].hit(&hits));
    }
}
