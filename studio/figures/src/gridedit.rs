//! (FIG-INTERACT) Editing a `grid … end` block in place.
//!
//! An edit from the window used to turn her ASCII art into statements at
//! the first touch. Now a changed grid object is *settled*: if `format` can
//! still write the block back verbatim (a node's new label or colour goes in
//! a statement after `end`), nothing happens to the art; otherwise small
//! patches of the art are tried (a relabelled or unlabelled arrow, a
//! vertical arrow's label moved to the other side of its bar, `|` ↔ `:`
//! for dashed, an arrow blanked out), and one is kept only if the patched
//! block parses to exactly the figure's objects. Anything the art can't say
//! (a move, a bend, a horizontal swap) still converts the block.
use crate::format::grid_keeps;
use crate::model::*;
use crate::parse;
use crate::spelling;

fn set_source(fig: &mut Figure, gi: usize, src: &str) {
    if let Some(Item::Grid { source, .. }) = fig.items.get_mut(gi) {
        *source = src.to_string();
    }
}

/// Keep grid `gi` as art: true if it can be written back verbatim (as it
/// is, or after a patch of the art, which is then in place).
pub(crate) fn settle(fig: &mut Figure, gi: usize) -> bool {
    let Some(Item::Grid { source, .. }) = fig.items.get(gi).cloned() else { return false };
    if grid_keeps(fig, gi) {
        return true;
    }
    for cand in candidates(fig, gi, &source) {
        set_source(fig, gi, &cand);
        if grid_keeps(fig, gi) {
            return true;
        }
    }
    set_source(fig, gi, &source);
    false
}

/// The arrows the grid should hold (its derived Arrow items, in order).
fn wanted(fig: &Figure, gi: usize) -> Vec<Arrow> {
    let mut d: Vec<usize> = fig.grid_items.iter().filter(|x| x.1 == gi).map(|x| x.0).collect();
    d.sort();
    d.into_iter().filter_map(|k| if let Item::Arrow(a) = &fig.items[k] { Some(a.clone()) } else { None }).collect()
}

/// A label as the art should spell it (her spelling; quoted when it has spaces or dashes).
fn art_label(fig: &Figure, a: &Arrow) -> Option<String> {
    let l = a.label.as_ref()?;
    let raw = spelling::raw(fig, &spelling::arrow_key(&a.from, &a.to), l);
    if raw.contains('"') {
        return None;
    }
    Some(if raw.contains(char::is_whitespace) || raw.contains('-') { format!("\"{raw}\"") } else { raw })
}

/// Maximal runs of non-space characters: (start, end) char columns.
fn runs(line: &[char]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < line.len() {
        if line[i].is_whitespace() {
            i += 1;
            continue;
        }
        let s = i;
        while i < line.len() && !line[i].is_whitespace() {
            i += 1;
        }
        out.push((s, i));
    }
    out
}

fn find_all(hay: &[char], needle: &[char]) -> Vec<usize> {
    if needle.is_empty() || needle.len() > hay.len() {
        return Vec::new();
    }
    (0..=hay.len() - needle.len()).filter(|&i| hay[i..i + needle.len()] == *needle).collect()
}

fn is_bar(c: char) -> bool {
    matches!(c, '|' | ':' | '‖')
}

/// Candidate sources for the difference between the art and the figure.
fn candidates(fig: &Figure, gi: usize, source: &str) -> Vec<String> {
    let lines: Vec<Vec<char>> = source.lines().map(|l| l.chars().collect()).collect();
    if lines.len() < 3 {
        return Vec::new();
    }
    // body: between the `grid` line and the `end` line
    let end = lines.iter().rposition(|l| l.iter().collect::<String>().trim() == "end").unwrap_or(lines.len());
    let body: Vec<usize> = (1..end).collect();
    let cur = parse::grid_from_source(source, 1);
    let want = wanted(fig, gi);
    let join = |ls: &Vec<Vec<char>>| -> String {
        let mut s: String = ls.iter().map(|l| l.iter().collect::<String>()).collect::<Vec<_>>().join("\n");
        if source.ends_with('\n') {
            s.push('\n');
        }
        s
    };
    let mut out: Vec<String> = Vec::new();
    let canon_l = |l: &Option<String>| l.as_ref().map(|s| crate::unicode::to_latex(s.trim()));
    if want.len() == cur.arrows.len() {
        let diff: Vec<usize> = (0..want.len())
            .filter(|&k| {
                let (w, c) = (&want[k], &cur.arrows[k]);
                w.label != canon_l(&c.label) || w.side != c.side || w.line != c.line_style
            })
            .collect();
        let [k] = diff.as_slice() else { return out };
        let (w, c) = (&want[*k], &cur.arrows[*k]);
        // a new label (or none)
        if w.label != canon_l(&c.label) {
            let new: Vec<char> = art_label(fig, w).unwrap_or_default().chars().collect();
            match &c.label {
                Some(old) => {
                    let old: Vec<char> = old.chars().collect();
                    let quoted: Vec<char> = format!("\"{}\"", old.iter().collect::<String>()).chars().collect();
                    for &li in &body {
                        for o in [&quoted, &old] {
                            for at in find_all(&lines[li], o) {
                                for v in relabel_variants(&lines, li, at, o.len(), &new) {
                                    out.push(join(&v));
                                }
                            }
                        }
                    }
                }
                None => {
                    for &li in &body {
                        for (s, e) in runs(&lines[li]) {
                            let run: String = lines[li][s..e].iter().collect();
                            // a bare vertical bar: the label goes right of it
                            if run.chars().all(is_bar) {
                                let mut l2 = lines.clone();
                                for (j, ch) in new.iter().enumerate() {
                                    let at = e + j;
                                    if at < l2[li].len() && l2[li][at].is_whitespace() && l2[li].get(at + 1).is_none_or(|c| c.is_whitespace()) {
                                        l2[li][at] = *ch;
                                    } else {
                                        l2[li].insert(at, *ch);
                                    }
                                }
                                out.push(join(&l2));
                            }
                            // a bare horizontal shaft: the label between its dashes
                            if run.contains('>') || run.contains('<') {
                                let cs: Vec<char> = run.chars().collect();
                                for cut in 1..cs.len() {
                                    if cs[cut - 1] == '-' && cs[cut] == '-' {
                                        let mut l2 = lines.clone();
                                        let mut ins: Vec<char> = new.clone();
                                        ins.push('-');
                                        ins.insert(0, '-');
                                        for (j, ch) in ins.iter().enumerate() {
                                            l2[li].insert(s + cut + j, *ch);
                                        }
                                        out.push(join(&l2));
                                        // keep the width: drop as many dashes as the label added
                                        let mut l3 = l2.clone();
                                        let mut extra = new.len() + 2;
                                        let p = s + cut + ins.len();
                                        while extra > 0 && p < l3[li].len() && l3[li][p] == '-' && l3[li].get(p + 1) == Some(&'-') {
                                            l3[li].remove(p);
                                            extra -= 1;
                                        }
                                        let mut p = s;
                                        while extra > 0 && p < l3[li].len() && l3[li][p] == '-' && l3[li][p + 1] == '-' {
                                            l3[li].remove(p);
                                            extra -= 1;
                                            p = s;
                                        }
                                        out.push(join(&l3));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            return out;
        }
        // the label to the other side of a vertical bar
        if w.side != c.side {
            for &li in &body {
                for (s, e) in runs(&lines[li]) {
                    let run: Vec<char> = lines[li][s..e].to_vec();
                    let n = run.len();
                    if n >= 2 && is_bar(run[0]) && !is_bar(run[1]) {
                        // `|g` → `g|`, the bar staying in its column
                        let lab = &run[1..];
                        if s >= lab.len() && lines[li][s - lab.len()..s].iter().all(|c| c.is_whitespace()) {
                            let mut l2 = lines.clone();
                            for (j, ch) in lab.iter().enumerate() {
                                l2[li][s - lab.len() + j] = *ch;
                            }
                            for j in 1..n {
                                l2[li][s + j] = ' ';
                            }
                            out.push(join(&l2));
                        }
                    } else if n >= 2 && is_bar(run[n - 1]) && !is_bar(run[n - 2]) {
                        // `g|` → `|g`
                        let lab = &run[..n - 1];
                        let bar = s + n - 1;
                        let mut l2 = lines.clone();
                        for j in 0..n - 1 {
                            l2[li][s + j] = ' ';
                        }
                        for (j, ch) in lab.iter().enumerate() {
                            let at = bar + 1 + j;
                            if at < l2[li].len() && l2[li][at].is_whitespace() && l2[li].get(at + 1).is_none_or(|c| c.is_whitespace()) {
                                l2[li][at] = *ch;
                            } else {
                                l2[li].insert(at, *ch);
                            }
                        }
                        out.push(join(&l2));
                    }
                }
            }
            return out;
        }
        // solid ↔ dashed: `|` ↔ `:` down one column
        if w.line != c.line_style {
            let (from, to) = match w.line {
                Line::Dashed => ('|', ':'),
                Line::Solid => (':', '|'),
                Line::Dotted => return out,
            };
            let mut cols: Vec<usize> = Vec::new();
            for &li in &body {
                for (s, e) in runs(&lines[li]) {
                    for p in [s, e - 1] {
                        if lines[li][p] == from && !cols.contains(&p) {
                            cols.push(p);
                        }
                    }
                }
            }
            for col in cols {
                let mut l2 = lines.clone();
                for &li in &body {
                    if l2[li].get(col) == Some(&from) {
                        l2[li][col] = to;
                    }
                }
                out.push(join(&l2));
            }
            return out;
        }
        return out;
    }
    // one arrow fewer: blank it out
    if want.len() + 1 == cur.arrows.len() {
        for &li in &body {
            for (s, e) in runs(&lines[li]) {
                let mut l2 = lines.clone();
                for p in s..e {
                    l2[li][p] = ' ';
                }
                out.push(join(&l2));
                // a vertical arrow spans lines: its other marks in (about) the same column
                let mut l3 = l2.clone();
                let mut more = false;
                for &lj in &body {
                    if lj == li {
                        continue;
                    }
                    for (s2, e2) in runs(&lines[lj]) {
                        let near = (s2 as isize - s as isize).abs() <= 1 || (e2 as isize - e as isize).abs() <= 1;
                        let mark: String = lines[lj][s2..e2].iter().collect();
                        let vertical = mark.chars().any(is_bar) || matches!(mark.as_str(), "v" | "V" | "^" | "↓" | "↑");
                        if near && vertical {
                            for p in s2..e2 {
                                l3[lj][p] = ' ';
                            }
                            more = true;
                        }
                    }
                }
                if more {
                    out.push(join(&l3));
                }
            }
        }
    }
    out
}

/// `old` (at `at`, `len` chars) replaced by `new` in line `li`, and the
/// ways to keep the columns lined up.
fn relabel_variants(lines: &[Vec<char>], li: usize, at: usize, len: usize, new: &[char]) -> Vec<Vec<Vec<char>>> {
    let mut out = Vec::new();
    let mut base = lines.to_vec();
    base[li].splice(at..at + len, new.iter().copied());
    let delta = new.len() as isize - len as isize;
    out.push(base.clone());
    if delta > 0 {
        let d = delta as usize;
        // a horizontal shaft: give back dashes on either side
        let mut l2 = base.clone();
        let mut left = d;
        let mut p = at;
        while left > 0 && p >= 2 && l2[li][p - 1] == '-' && l2[li][p - 2] == '-' {
            l2[li].remove(p - 1);
            p -= 1;
            left -= 1;
        }
        let q = p + new.len();
        while left > 0 && q + 1 < l2[li].len() && l2[li][q] == '-' && l2[li][q + 1] == '-' {
            l2[li].remove(q);
            left -= 1;
        }
        if left < d {
            out.push(l2);
        }
        // a vertical label: take the room from the spaces after it
        let mut l3 = base.clone();
        let q = at + new.len();
        let mut left = d;
        while left > 0 && q + 1 < l3[li].len() && l3[li][q].is_whitespace() && l3[li][q + 1].is_whitespace() {
            l3[li].remove(q);
            left -= 1;
        }
        if left < d {
            out.push(l3);
        }
        // or widen the column: the same room in every other line
        let mut l4 = base.clone();
        let col = at + len;
        let mut ok = true;
        for (k, l) in l4.iter_mut().enumerate() {
            if k == li || k == 0 || k + 1 == lines.len() {
                continue;
            }
            if col >= l.len() {
                continue;
            }
            // inside another shaft: more shaft; between things: more space
            let fill = if l[col] == '-' || (col > 0 && l[col - 1] == '-' && !l[col].is_whitespace()) { '-' } else if l[col].is_whitespace() || (col > 0 && l[col - 1].is_whitespace()) { ' ' } else {
                ok = false;
                break;
            };
            for _ in 0..d {
                l.insert(col, fill);
            }
        }
        if ok {
            out.push(l4);
        }
    } else if delta < 0 {
        let d = (-delta) as usize;
        // pad: dashes in a shaft, spaces after a vertical label
        let mut l2 = base.clone();
        let q = at + new.len();
        let fill = if l2[li].get(q) == Some(&'-') || (q > 0 && l2[li].get(q - 1) == Some(&'-')) { '-' } else { ' ' };
        for _ in 0..d {
            l2[li].insert(q, fill);
        }
        out.push(l2);
        if new.is_empty() {
            // a label removed from `--f-->` leaves `---->`; from `|g`, `|`
            let mut l3 = base.clone();
            for _ in 0..d {
                l3[li].insert(at, '-');
            }
            out.push(l3);
        }
    }
    out
}
