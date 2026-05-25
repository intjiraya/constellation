use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use serde::Serialize;

const MIN_TOKEN_LEN: usize = 2;
const CLUSTER_GAP: usize = 80;
const SNIPPET_CONTEXT: usize = 80;
const MAX_SNIPPETS_PER_HIT: usize = 3;

#[derive(Debug, Default, Clone, Serialize)]
pub struct Snippet {
    pub text: String,
    pub matches: Vec<(usize, usize)>,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct SearchHit {
    pub session_id: String,
    pub score: u32,
    pub snippets: Vec<Snippet>,
}

#[derive(Debug, Clone)]
struct IndexedDoc {
    session_id: String,
    chars: Vec<char>,
    lowered: Vec<char>,
}

#[derive(Debug, Default)]
struct SuffixIndex {
    entries: Vec<(String, u32)>,
    tokens: Vec<String>,
}

#[derive(Debug, Default)]
pub struct SearchIndex {
    postings: HashMap<String, Vec<u32>>,
    docs: Vec<IndexedDoc>,
    suffix: OnceLock<SuffixIndex>,
}

impl Clone for SearchIndex {
    fn clone(&self) -> Self {
        Self {
            postings: self.postings.clone(),
            docs: self.docs.clone(),
            suffix: OnceLock::new(),
        }
    }
}

impl SearchIndex {
    pub fn add(&mut self, session_id: impl Into<String>, body: impl Into<String>) {
        let id = session_id.into();
        let body = body.into();
        let idx = self.docs.len() as u32;

        let mut seen = HashSet::new();
        for tok in tokenize_text(&body) {
            if seen.insert(tok.clone()) {
                self.postings.entry(tok).or_default().push(idx);
            }
        }

        let chars: Vec<char> = body.chars().collect();
        let lowered: Vec<char> = chars
            .iter()
            .map(|c| c.to_lowercase().next().unwrap_or(*c))
            .collect();
        self.docs.push(IndexedDoc {
            session_id: id,
            chars,
            lowered,
        });
    }

    pub fn search(&self, terms: &[String], limit: usize) -> Vec<SearchHit> {
        let normalized: Vec<String> = terms
            .iter()
            .map(|t| t.to_lowercase())
            .filter(|t| t.chars().count() >= MIN_TOKEN_LEN)
            .collect();
        if normalized.is_empty() {
            return Vec::new();
        }

        let suffix = self.suffix.get_or_init(|| self.build_suffix_index());

        let mut acc: Option<HashSet<u32>> = None;
        for term in &normalized {
            let posts = lookup_by_substring(suffix, &self.postings, term);
            acc = Some(match acc {
                None => posts,
                Some(prev) => prev.intersection(&posts).copied().collect(),
            });
        }
        let candidates = acc.unwrap_or_default();

        let mut hits: Vec<SearchHit> = candidates
            .into_iter()
            .map(|idx| {
                let doc = &self.docs[idx as usize];
                let snippets = build_snippets_from_chars(&doc.chars, &doc.lowered, &normalized);
                let score: u32 = snippets.iter().map(|s| s.matches.len() as u32).sum();
                SearchHit {
                    session_id: doc.session_id.clone(),
                    score,
                    snippets,
                }
            })
            .collect();

        hits.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.session_id.cmp(&b.session_id))
        });
        hits.truncate(limit);
        hits
    }

    fn build_suffix_index(&self) -> SuffixIndex {
        let tokens: Vec<String> = self.postings.keys().cloned().collect();
        let mut entries: Vec<(String, u32)> = Vec::new();
        for (idx, tok) in tokens.iter().enumerate() {
            let chars: Vec<char> = tok.chars().collect();
            for start in 0..chars.len() {
                let suffix: String = chars[start..].iter().collect();
                entries.push((suffix, idx as u32));
            }
        }
        entries.sort();
        SuffixIndex { entries, tokens }
    }
}

fn lookup_by_substring(
    suffix: &SuffixIndex,
    postings: &HashMap<String, Vec<u32>>,
    term: &str,
) -> HashSet<u32> {
    let start = suffix.entries.partition_point(|(s, _)| s.as_str() < term);
    let mut token_ids: HashSet<u32> = HashSet::new();
    for (s, tid) in suffix.entries[start..].iter() {
        if !s.starts_with(term) {
            break;
        }
        token_ids.insert(*tid);
    }
    let mut docs: HashSet<u32> = HashSet::new();
    for tid in token_ids {
        if let Some(post) = postings.get(&suffix.tokens[tid as usize]) {
            docs.extend(post.iter().copied());
        }
    }
    docs
}

fn build_snippets_from_chars(chars: &[char], lower: &[char], terms: &[String]) -> Vec<Snippet> {
    if chars.is_empty() {
        return Vec::new();
    }

    let mut hits: Vec<(usize, usize)> = Vec::new();
    for term in terms {
        let needle: Vec<char> = term.chars().collect();
        if needle.is_empty() || needle.len() > lower.len() {
            continue;
        }
        for i in 0..=(lower.len() - needle.len()) {
            if lower[i..i + needle.len()] == needle[..] {
                hits.push((i, i + needle.len()));
            }
        }
    }
    if hits.is_empty() {
        return Vec::new();
    }

    hits.sort();
    let mut merged: Vec<(usize, usize)> = vec![hits[0]];
    for &(s, e) in &hits[1..] {
        let last = merged.last_mut().unwrap();
        if s <= last.1 {
            if e > last.1 {
                last.1 = e;
            }
        } else {
            merged.push((s, e));
        }
    }

    let mut clusters: Vec<Vec<(usize, usize)>> = Vec::new();
    for m in merged {
        match clusters.last_mut() {
            Some(c) if m.0.saturating_sub(c.last().unwrap().1) <= CLUSTER_GAP => {
                c.push(m);
            }
            _ => clusters.push(vec![m]),
        }
    }

    clusters
        .into_iter()
        .take(MAX_SNIPPETS_PER_HIT)
        .map(|cluster| {
            let first = cluster.first().unwrap().0;
            let last = cluster.last().unwrap().1;
            let start = first.saturating_sub(SNIPPET_CONTEXT);
            let end = (last + SNIPPET_CONTEXT).min(chars.len());
            let text: String = chars[start..end].iter().collect();
            let matches: Vec<(usize, usize)> = cluster
                .iter()
                .map(|&(s, e)| (s - start, e - start))
                .collect();
            Snippet { text, matches }
        })
        .collect()
}

pub fn tokenize_text(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = String::new();
    for ch in input.chars() {
        if ch.is_alphanumeric() {
            for c in ch.to_lowercase() {
                buf.push(c);
            }
        } else if buf.chars().count() >= MIN_TOKEN_LEN {
            out.push(std::mem::take(&mut buf));
        } else {
            buf.clear();
        }
    }
    if buf.chars().count() >= MIN_TOKEN_LEN {
        out.push(buf);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_text_lowercases_and_splits_on_non_alnum() {
        assert_eq!(
            tokenize_text("Hello, World!  it's-AUTH bug"),
            vec!["hello", "world", "it", "auth", "bug"],
        );
    }

    #[test]
    fn tokenize_text_drops_single_char_tokens() {
        assert_eq!(tokenize_text("a b cd e"), vec!["cd"]);
    }

    #[test]
    fn tokenize_text_handles_empty_and_whitespace() {
        assert_eq!(tokenize_text(""), Vec::<String>::new());
        assert_eq!(tokenize_text("   "), Vec::<String>::new());
        assert_eq!(tokenize_text("!@#$%"), Vec::<String>::new());
    }

    #[test]
    fn tokenize_text_supports_unicode() {
        assert_eq!(tokenize_text("Привет, МИР!"), vec!["привет", "мир"],);
    }

    #[test]
    fn tokenize_text_keeps_alphanumeric_words_with_digits() {
        assert_eq!(
            tokenize_text("file42 ver1.10"),
            vec!["file42", "ver1", "10"]
        );
    }

    #[test]
    fn tokenize_text_drops_short_numeric_tails() {
        assert_eq!(tokenize_text("v1.0"), vec!["v1"]);
    }

    fn ids(hits: &[SearchHit]) -> Vec<&str> {
        hits.iter().map(|h| h.session_id.as_str()).collect()
    }

    #[test]
    fn index_empty_returns_no_hits() {
        let idx = SearchIndex::default();
        let q = ["auth".to_string()];
        assert!(idx.search(&q, 50).is_empty());
    }

    #[test]
    fn index_single_session_single_term_hit() {
        let mut idx = SearchIndex::default();
        idx.add("s1", "Authentication is broken");
        let hits = idx.search(&["auth".to_string()], 50);
        assert_eq!(ids(&hits), vec!["s1"]);
    }

    #[test]
    fn index_multi_term_is_and_across_terms() {
        let mut idx = SearchIndex::default();
        idx.add("s1", "Authentication bug fix");
        idx.add("s2", "Authentication only");
        idx.add("s3", "Just bug");
        let hits = idx.search(&["auth".to_string(), "bug".to_string()], 50);
        let mut got = ids(&hits);
        got.sort();
        assert_eq!(got, vec!["s1"]);
    }

    #[test]
    fn index_substring_of_token_matches() {
        let mut idx = SearchIndex::default();
        idx.add("s1", "Authentication broken");
        idx.add("s2", "Authorization missing");
        let hits = idx.search(&["auth".to_string()], 50);
        let mut got = ids(&hits);
        got.sort();
        assert_eq!(got, vec!["s1", "s2"]);
    }

    #[test]
    fn index_case_insensitive() {
        let mut idx = SearchIndex::default();
        idx.add("s1", "AUTH bug");
        let hits = idx.search(&["AuTh".to_string()], 50);
        assert_eq!(ids(&hits), vec!["s1"]);
    }

    #[test]
    fn index_drops_short_query_terms_silently() {
        let mut idx = SearchIndex::default();
        idx.add("s1", "auth bug");
        let hits = idx.search(&["a".to_string(), "auth".to_string()], 50);
        assert_eq!(ids(&hits), vec!["s1"]);
    }

    #[test]
    fn search_hit_carries_a_snippet_with_match_offsets() {
        let mut idx = SearchIndex::default();
        idx.add("s1", "We need to fix the auth bug today");
        let hits = idx.search(&["auth".to_string()], 50);
        assert_eq!(hits.len(), 1);
        let snip = hits[0].snippets.first().expect("at least one snippet");
        assert!(snip.text.to_lowercase().contains("auth"));
        assert!(!snip.matches.is_empty());
        let (start, end) = snip.matches[0];
        let snip_chars: Vec<char> = snip.text.chars().collect();
        assert!(end <= snip_chars.len());
        let matched: String = snip_chars[start..end].iter().collect();
        assert_eq!(
            matched.to_lowercase(),
            "auth",
            "snippet.matches[0] should bracket the matched term, got {matched:?}",
        );
    }

    #[test]
    fn snippet_offset_at_document_start() {
        let mut idx = SearchIndex::default();
        idx.add(
            "s1",
            "auth followed by lots of filler text here filling space",
        );
        let hits = idx.search(&["auth".to_string()], 50);
        let snip = &hits[0].snippets[0];
        let (start, end) = snip.matches[0];
        let chars: Vec<char> = snip.text.chars().collect();
        let matched: String = chars[start..end].iter().collect();
        assert_eq!(matched.to_lowercase(), "auth");
        assert_eq!(start, 0, "term at position 0 should land at snippet start");
    }

    #[test]
    fn snippet_offset_at_document_end() {
        let mut idx = SearchIndex::default();
        idx.add(
            "s1",
            "this document is mostly filler and ends with the marker auth",
        );
        let hits = idx.search(&["auth".to_string()], 50);
        let snip = &hits[0].snippets[0];
        let (start, end) = snip.matches[0];
        let chars: Vec<char> = snip.text.chars().collect();
        let matched: String = chars[start..end].iter().collect();
        assert_eq!(matched.to_lowercase(), "auth");
        assert_eq!(
            end,
            chars.len(),
            "term at document end should land at snippet end",
        );
    }

    #[test]
    fn search_results_sorted_by_score_desc() {
        let mut idx = SearchIndex::default();
        idx.add("a", "auth auth auth bug");
        idx.add("b", "auth bug");
        let hits = idx.search(&["auth".to_string()], 50);
        assert_eq!(hits[0].session_id, "a");
        assert!(hits[0].score >= hits[1].score);
    }

    #[test]
    fn search_respects_limit() {
        let mut idx = SearchIndex::default();
        for i in 0..10 {
            idx.add(format!("s{i}"), "auth bug");
        }
        let hits = idx.search(&["auth".to_string()], 3);
        assert_eq!(hits.len(), 3);
    }

    fn fixture_path(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sessions")
            .join(name)
    }

    #[test]
    fn build_index_from_session_finds_internal_terms() {
        let session = crate::parser::parse_session(&fixture_path("with_tools.jsonl"));
        let mut idx = SearchIndex::default();
        idx.add(&session.meta.id, session.indexable_text());

        let hits = idx.search(&["readme".to_string()], 50);
        assert_eq!(
            hits.len(),
            1,
            "should match the README snippet inside tool output"
        );
        assert_eq!(hits[0].session_id, session.meta.id);
    }
}
