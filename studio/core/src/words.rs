//! English words, for "this trigger would fire while you type ‘offer’".
//! /usr/share/dict/words (plus a short built-in list of common words, so a
//! missing dictionary still warns), loaded once.
//!
//! Triggers are case-sensitive, and so is everything here: `the` starts
//! "then", `The` starts "Thebes". Only word *starts* matter: with
//! `fire_after_letters = false` (her prefs) a letter trigger never fires in the
//! middle of a word.
use std::sync::OnceLock;

/// Where the dictionary lives. `NTEX_STUDIO_WORDS` overrides it (tests point
/// it at a missing file to check the built-in fallback).
const DICT: &str = "/usr/share/dict/words";

/// Very common everyday words, most common first. They rank first among the
/// words a trigger starts, and they are the fallback when there is no
/// dictionary. Hand-written; ordinary words only.
const COMMON: &[&str] = &[
    "the", "of", "and", "to", "in", "is", "it", "that", "was", "for", "on", "are", "as", "with", "be", "this", "have",
    "from", "or", "one", "had", "by", "but", "not", "what", "all", "were", "we", "when", "your", "can", "said",
    "there", "use", "an", "each", "which", "she", "do", "how", "their", "if", "will", "up", "other", "about", "out",
    "many", "then", "them", "these", "so", "some", "her", "would", "make", "like", "him", "into", "time", "has",
    "look", "two", "more", "write", "go", "see", "number", "no", "way", "could", "my", "than", "first", "been",
    "call", "now", "find", "long", "down", "day", "made", "may", "part", "over", "new", "take", "only", "work",
    "know", "place", "back", "most", "very", "after", "thing", "just", "name", "think", "where", "through", "much",
    "before", "line", "right", "too", "mean", "same", "follow", "want", "show", "also", "around", "form", "set",
    "put", "end", "does", "well", "even", "such", "because", "here", "need", "different", "move", "try", "change",
    "again", "point", "should", "between", "offer", "effect", "begin", "example", "every", "important", "next",
    "open", "problem", "question", "since", "sure", "theorem", "proof", "suppose", "section", "item",
];

struct Words {
    /// Every word, byte-sorted, no duplicates.
    sorted: Vec<&'static str>,
}

static WORDS: OnceLock<Words> = OnceLock::new();

fn load() -> Words {
    let path = std::env::var("NTEX_STUDIO_WORDS").unwrap_or_else(|_| DICT.to_string());
    // read once, kept for the life of the process
    let text: &'static str = match std::fs::read_to_string(&path) {
        Ok(t) => Box::leak(t.into_boxed_str()),
        Err(_) => "",
    };
    let mut sorted: Vec<&'static str> = text.lines().map(str::trim).filter(|w| !w.is_empty()).collect();
    sorted.extend_from_slice(COMMON);
    sorted.sort_unstable();
    sorted.dedup();
    Words { sorted }
}

fn words() -> &'static Words {
    WORDS.get_or_init(load)
}

/// Load the dictionary now (blocking), so the first keystroke that needs it
/// doesn't pay for reading it.
pub fn preload() {
    let _ = words();
}

/// Load it on a background thread (call once at startup). Reading and
/// sorting the 236k words takes ~150 ms in a debug build, which must not
/// land on the first keystroke of the UI thread.
pub fn preload_in_background() {
    if WORDS.get().is_none() {
        let _ = std::thread::Builder::new().name("ntex-words".into()).spawn(preload);
    }
}

/// Rank among the common words (0 = most common), if it is one.
pub fn common_rank(w: &str) -> Option<usize> {
    COMMON.iter().position(|c| *c == w)
}

/// The words that start with `prefix` and are longer than it, unsorted.
fn longer(prefix: &str) -> &'static [&'static str] {
    if prefix.is_empty() {
        return &[];
    }
    let w = &words().sorted;
    let from = w.partition_point(|x| *x <= prefix);
    let to = from + w[from..].partition_point(|x| x.starts_with(prefix));
    &w[from..to]
}

/// Common words that start with `prefix` (case-sensitive, as triggers are),
/// most common first, at most `limit`.
///
/// "Most common" is the built-in list's order; the rest follow, shortest
/// first, then alphabetically. `prefix` itself is not included (`is_word`
/// answers that).
pub fn starting_with(prefix: &str, limit: usize) -> Vec<String> {
    let mut v: Vec<&str> = longer(prefix).to_vec();
    v.sort_by(|a, b| {
        let ra = common_rank(a).unwrap_or(usize::MAX);
        let rb = common_rank(b).unwrap_or(usize::MAX);
        (ra, a.len(), *a).cmp(&(rb, b.len(), *b))
    });
    v.into_iter().take(limit).map(str::to_string).collect()
}

/// How many words start with `prefix` (longer than it).
pub fn count_starting_with(prefix: &str) -> usize {
    longer(prefix).len()
}

/// Does `prefix` start ordinary words often enough to get in the way? Yes
/// when it starts one of the common words, or at least 3 dictionary words
/// (one or two obscure ones don't count).
pub fn starts_common_words(prefix: &str) -> bool {
    let l = longer(prefix);
    l.len() >= 3 || l.iter().any(|w| common_rank(w).is_some())
}

/// Is `s` itself a word?
pub fn is_word(s: &str) -> bool {
    !s.is_empty() && words().sorted.binary_search(&s).is_ok()
}
