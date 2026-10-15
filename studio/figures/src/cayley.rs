//! Groups for Cayley graphs: named families and coset enumeration
//! (Todd–Coxeter) for presentations; elements as shortlex words.
//!
//! Conventions (FIG-CORE-B):
//! - Elements are found by breadth-first search from the identity over right
//!   multiplication, trying the requested generators in the order given.
//!   That order is exactly shortlex on positive words (length, then
//!   lexicographic by generator order), so each element's label is its
//!   shortlex-least positive word.
//! - Labels are LaTeX maths. The identity is `e`. Runs collapse into powers:
//!   `r·r·s` → `r^2s`; exponents of 10 or more are braced (`r^{10}`).
//!   Names are juxtaposed when every generator name is one character or a
//!   parenthesised cycle (`rs`, `(12)(123)`); otherwise they are separated by
//!   a space (`a1 a2^2`). A powered name that isn't atomic is wrapped:
//!   `(-1)^2`.
//! - Additive labels: `Zn`/`Cn` with numeric generators (`<1>`, `<2, 3>`) are
//!   labelled by residues (`0`, `1`, `2`, …); products `Zm x Zn` with tuple
//!   generators (`<(1,0), (0,1)>`) by tuples (`(0,0)`, `(1,0)`, …). Element 0
//!   is still the identity.
//! - If the requested generators don't generate the whole group, the result
//!   is the subgroup they generate (still a correct Cayley graph of it).
//! - At most 96 elements ([`MAX_ORDER`]); larger groups are an error.
use crate::model::GroupSpec;
use std::collections::HashMap;

/// The most elements a Cayley graph is drawn with.
pub const MAX_ORDER: usize = 96;
/// Coset enumeration gives up when this many cosets are live at once.
pub const MAX_LIVE_COSETS: usize = 20_000;
/// …or when this many cosets have been defined in total.
const MAX_DEFINED_COSETS: usize = 400_000;

pub struct Group {
    /// shortlex words ("e", "r", "r^2", "s", "rs", …), identity first
    pub elements: Vec<String>,
    /// for each generator (in order): right-multiplication table, elements[i]·g = elements[table[i]]
    pub gens: Vec<(String, Vec<usize>)>,
}

impl Group {
    /// Number of elements.
    pub fn order(&self) -> usize {
        self.elements.len()
    }
    /// Whether generator `k` is an involution (g² = e, g ≠ e): its edges can
    /// be drawn once, with no head.
    pub fn is_involution(&self, k: usize) -> bool {
        let t = &self.gens[k].1;
        t[0] != 0 && (0..t.len()).all(|i| t[t[i]] == i)
    }
    /// Whether the requested generators commute (so the group is abelian).
    pub fn is_abelian(&self) -> bool {
        self.gens.iter().all(|(_, a)| {
            self.gens.iter().all(|(_, b)| (0..a.len()).all(|i| b[a[i]] == a[b[i]]))
        })
    }
}

/// The shape of a known group, for automatic layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Cn / Zn
    Cyclic(usize),
    /// Dn (order 2n)
    Dihedral(usize),
    /// Zm x Zn x … (V4 = [2, 2])
    Product(Vec<usize>),
    /// Q8, Sn, An, or a presentation
    Other,
}

/// The family of a group spec (presentations are `Other`; unknown names too).
pub fn kind(spec: &GroupSpec) -> Kind {
    match spec {
        GroupSpec::Named(name) => match parse_name(name) {
            Ok(Named::Cyclic(n)) => Kind::Cyclic(n),
            Ok(Named::Dihedral(n)) => Kind::Dihedral(n),
            Ok(Named::Product(f)) => Kind::Product(f),
            _ => Kind::Other,
        },
        GroupSpec::Presentation(..) => Kind::Other,
    }
}

pub fn group(spec: &GroupSpec, gens: &[String]) -> Result<Group, String> {
    match spec {
        GroupSpec::Named(name) => named_group(name, gens),
        GroupSpec::Presentation(pg, rels) => presented_group(pg, rels, gens),
    }
}

// ---------------------------------------------------------------------------
// breadth-first search in shortlex order

/// BFS from `start`; `step(k, g)` is element k's key times generator g.
/// Returns the keys in shortlex order, the tables, and each element's letters.
fn bfs<K: Clone + Eq + std::hash::Hash>(
    start: K,
    ngens: usize,
    mut step: impl FnMut(&K, usize) -> K,
) -> Result<(Vec<K>, Vec<Vec<usize>>, Vec<Vec<usize>>), String> {
    let mut keys = vec![start.clone()];
    let mut index: HashMap<K, usize> = HashMap::new();
    index.insert(start, 0);
    let mut words: Vec<Vec<usize>> = vec![vec![]];
    let mut tables: Vec<Vec<usize>> = vec![Vec::new(); ngens];
    let mut i = 0;
    while i < keys.len() {
        for g in 0..ngens {
            let next = step(&keys[i], g);
            let j = match index.get(&next) {
                Some(&j) => j,
                None => {
                    if keys.len() >= MAX_ORDER {
                        return Err(too_big());
                    }
                    let j = keys.len();
                    index.insert(next.clone(), j);
                    keys.push(next);
                    let mut w = words[i].clone();
                    w.push(g);
                    words.push(w);
                    j
                }
            };
            tables[g].push(j);
        }
        i += 1;
    }
    Ok((keys, tables, words))
}

fn too_big() -> String {
    format!("the group has more than {MAX_ORDER} elements (Cayley graphs are drawn up to {MAX_ORDER})")
}

/// A generator name that reads as one symbol (so `name^2` is unambiguous).
fn atomic(name: &str) -> bool {
    let mut cs = name.chars();
    let Some(c0) = cs.next() else { return true };
    if name.chars().count() == 1 {
        return true;
    }
    if name.starts_with('(') && name.ends_with(')') {
        return true;
    }
    if name.starts_with('\\') {
        // \sigma, \sigma_1
        let rest: String = cs.collect();
        let cmd: String = rest.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
        let tail = &rest[cmd.len()..];
        return !cmd.is_empty() && subscript_like(tail);
    }
    c0.is_alphabetic() && subscript_like(cs.as_str())
}

fn subscript_like(tail: &str) -> bool {
    tail.chars().all(|c| c.is_ascii_digit() || matches!(c, '_' | '{' | '}' | '\''))
}

/// Writes a letter sequence (indices into `names`) as a word with powers.
fn word_label(letters: &[usize], names: &[String]) -> String {
    if letters.is_empty() {
        return "e".into();
    }
    let juxtapose = names.iter().all(|n| n.chars().count() == 1 || (n.starts_with('(') && n.ends_with(')')));
    let mut parts: Vec<String> = Vec::new();
    let mut i = 0;
    while i < letters.len() {
        let g = letters[i];
        let mut k = 1;
        while i + k < letters.len() && letters[i + k] == g {
            k += 1;
        }
        let name = &names[g];
        let part = if k == 1 {
            name.clone()
        } else {
            let base = if atomic(name) { name.clone() } else { format!("({name})") };
            if k < 10 { format!("{base}^{k}") } else { format!("{base}^{{{k}}}") }
        };
        parts.push(part);
        i += k;
    }
    parts.join(if juxtapose { "" } else { " " })
}

fn finish(names: &[String], tables: Vec<Vec<usize>>, elements: Vec<String>) -> Group {
    Group { elements, gens: names.iter().cloned().zip(tables).collect() }
}

// ---------------------------------------------------------------------------
// named groups, as permutation groups (right action: (p·q)(x) = q[p[x]])

type Perm = Vec<u32>;

fn compose(p: &Perm, q: &Perm) -> Perm {
    p.iter().map(|&x| q[x as usize]).collect()
}

#[derive(Clone, Debug, PartialEq)]
enum Named {
    Cyclic(usize),
    Dihedral(usize),
    Product(Vec<usize>),
    Quaternion,
    Symmetric(usize),
    Alternating(usize),
}

const KNOWN: &str = "known: Cn, Zn, Dn, V4, Q8, Sn, An, Zm x Zn";
/// Families with more points than this are refused before building anything.
const MAX_DEGREE: usize = 100_000;

fn parse_name(raw: &str) -> Result<Named, String> {
    let unknown = || format!("unknown group `{}`; {KNOWN}", raw.trim());
    let mut s: String = raw
        .replace("\\times", "x")
        .replace("\\mathbb{Z}", "z")
        .replace("\\mathbbZ", "z")
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '_' | '{' | '}' | '$'))
        .map(|c| match c {
            '×' => 'x',
            'ℤ' => 'z',
            _ => c,
        })
        .collect::<String>()
        .to_lowercase();
    // Z/nZ → zn
    if let Some(stripped) = s.strip_prefix("z/").and_then(|r| r.strip_suffix('z')) {
        s = format!("z{stripped}");
    }
    let num = |t: &str| -> Option<usize> {
        if t.is_empty() || !t.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        t.parse::<usize>().ok()
    };
    match s.as_str() {
        "v4" | "v" | "klein" | "k4" => return Ok(Named::Product(vec![2, 2])),
        "q8" | "q" => return Ok(Named::Quaternion),
        _ => {}
    }
    let parts: Vec<&str> = s.split('x').collect();
    if parts.len() > 1 {
        let mut f = Vec::new();
        for p in &parts {
            let n = p
                .strip_prefix('z')
                .or_else(|| p.strip_prefix('c'))
                .and_then(num)
                .ok_or_else(|| format!("unknown group `{}`: a product is written like Z2 x Z3; {KNOWN}", raw.trim()))?;
            if n == 0 {
                return Err(format!("`{}`: Z0 is infinite", raw.trim()));
            }
            f.push(n);
        }
        if f.iter().sum::<usize>() > MAX_DEGREE {
            return Err(too_big());
        }
        return Ok(Named::Product(f));
    }
    let (head, tail) = s.split_at(s.find(|c: char| c.is_ascii_digit()).unwrap_or(s.len()));
    let n = num(tail).ok_or_else(unknown)?;
    if n == 0 {
        return Err(format!("`{}` needs n ≥ 1", raw.trim()));
    }
    let named = match head {
        "c" | "z" => Named::Cyclic(n),
        "d" | "dih" => Named::Dihedral(n),
        "s" | "sym" => Named::Symmetric(n),
        "a" | "alt" => Named::Alternating(n),
        _ => return Err(unknown()),
    };
    if n > MAX_DEGREE {
        return Err(too_big());
    }
    Ok(named)
}

/// What a family knows how to name.
struct Family {
    /// display name for messages
    title: String,
    degree: usize,
    /// standard generators: (default name, perm)
    standard: Vec<(String, Perm)>,
    /// letter names that words may be built from (r, s; i, j, k)
    letters: Vec<(String, Perm)>,
}

fn cycle_perm(n: usize, cycles: &[Vec<usize>]) -> Perm {
    let mut p: Perm = (0..n as u32).collect();
    for c in cycles {
        for k in 0..c.len() {
            p[c[k]] = c[(k + 1) % c.len()] as u32;
        }
    }
    p
}

fn is_even(p: &Perm) -> bool {
    let mut seen = vec![false; p.len()];
    let mut transpositions = 0;
    for i in 0..p.len() {
        if !seen[i] {
            let mut j = i;
            let mut len = 0;
            while !seen[j] {
                seen[j] = true;
                j = p[j] as usize;
                len += 1;
            }
            transpositions += len - 1;
        }
    }
    transpositions % 2 == 0
}

/// "(12)(34)", "(1 2 3)", "(1,2,3)", "()" → cycles of 0-based points.
fn parse_cycles(s: &str, n: usize, title: &str) -> Option<Result<Vec<Vec<usize>>, String>> {
    let t = s.trim();
    if !t.starts_with('(') || !t.ends_with(')') {
        return None;
    }
    let mut cycles = Vec::new();
    for chunk in t.split('(').skip(1) {
        let body = chunk.trim().strip_suffix(')')?;
        let pts: Vec<usize> = if body.contains(',') || body.contains(' ') {
            let mut v = Vec::new();
            for x in body.split(|c: char| c == ',' || c.is_whitespace()).filter(|x| !x.is_empty()) {
                v.push(x.parse::<usize>().ok()?);
            }
            v
        } else {
            body.chars().map(|c| c.to_digit(10).map(|d| d as usize)).collect::<Option<Vec<_>>>()?
        };
        let mut seen = Vec::new();
        for &p in &pts {
            if p == 0 || p > n {
                return Some(Err(format!("`{t}` moves {p}, but {title} acts on 1..{n}")));
            }
            if seen.contains(&p) || cycles.iter().any(|c: &Vec<usize>| c.contains(&(p - 1))) {
                return Some(Err(format!("`{t}` repeats the point {p}")));
            }
            seen.push(p);
        }
        cycles.push(pts.into_iter().map(|p| p - 1).collect());
    }
    Some(Ok(cycles))
}

fn family(named: &Named) -> Family {
    let s = |x: &str| x.to_string();
    match named {
        Named::Cyclic(n) => {
            let n = *n;
            let a = cycle_perm(n, &[(0..n).collect()]);
            Family { title: format!("Z{n}"), degree: n, standard: vec![(s("a"), a)], letters: vec![] }
        }
        Named::Dihedral(n) => {
            // the n-gon's vertices, plus two points that record orientation
            // (so D1 and D2 are faithful too)
            let n = *n;
            let mut r: Perm = (0..n as u32).map(|x| (x + 1) % n as u32).collect();
            r.extend([n as u32, n as u32 + 1]);
            let mut f: Perm = (0..n as u32).map(|x| (n as u32 - x) % n as u32).collect();
            f.extend([n as u32 + 1, n as u32]);
            let std = vec![(s("r"), r), (s("s"), f)];
            Family { title: format!("D{n}"), degree: n + 2, standard: std.clone(), letters: std }
        }
        Named::Product(fs) => {
            let deg: usize = fs.iter().sum();
            let mut std = Vec::new();
            let mut base = 0;
            for (k, &m) in fs.iter().enumerate() {
                let p = cycle_perm(deg, &[(base..base + m).collect()]);
                let name = ((b'a' + (k % 26) as u8) as char).to_string();
                std.push((name, p));
                base += m;
            }
            let title = fs.iter().map(|m| format!("Z{m}")).collect::<Vec<_>>().join("x");
            Family { title, degree: deg, standard: std, letters: vec![] }
        }
        Named::Quaternion => {
            let q = |b: usize| quaternion_perm(b);
            let letters = vec![(s("i"), q(1)), (s("j"), q(2)), (s("k"), q(3))];
            Family { title: s("Q8"), degree: 8, standard: letters[..2].to_vec(), letters }
        }
        Named::Symmetric(n) => {
            let n = *n;
            let mut std = Vec::new();
            if n >= 3 {
                std.push((s("a"), cycle_perm(n, &[(0..n).collect()])));
            }
            if n >= 2 {
                std.push((if n >= 3 { s("b") } else { s("a") }, cycle_perm(n, &[vec![0, 1]])));
            }
            Family { title: format!("S{n}"), degree: n, standard: std, letters: vec![] }
        }
        Named::Alternating(n) => {
            let n = *n;
            let mut std = Vec::new();
            if n >= 3 {
                std.push((s("a"), cycle_perm(n, &[vec![0, 1, 2]])));
            }
            if n == 4 {
                std.push((s("b"), cycle_perm(n, &[vec![0, 1], vec![2, 3]])));
            } else if n >= 5 {
                let c: Vec<usize> = if n % 2 == 1 { (0..n).collect() } else { (1..n).collect() };
                std.push((s("b"), cycle_perm(n, &[c])));
            }
            Family { title: format!("A{n}"), degree: n, standard: std, letters: vec![] }
        }
    }
}

/// Right-regular permutation of the quaternion basis element `b` (0=1, 1=i,
/// 2=j, 3=k) on the 8 points sign·4 + basis.
fn quaternion_perm(b: usize) -> Perm {
    // basis products x·y = (sign, basis)
    const T: [[(u32, u32); 4]; 4] = [
        [(0, 0), (0, 1), (0, 2), (0, 3)],
        [(0, 1), (1, 0), (0, 3), (1, 2)],
        [(0, 2), (1, 3), (1, 0), (0, 1)],
        [(0, 3), (0, 2), (1, 1), (1, 0)],
    ];
    (0..8)
        .map(|p: u32| {
            let (sx, x) = (p / 4, p % 4);
            let (sg, y) = T[x as usize][b];
            ((sx + sg) % 2) * 4 + y
        })
        .collect()
}

fn named_group(name: &str, gens: &[String]) -> Result<Group, String> {
    let named = parse_name(name)?;
    let fam = family(&named);
    let deg = fam.degree;
    let ident: Perm = (0..deg as u32).collect();

    let names: Vec<String> = if gens.is_empty() {
        match &named {
            Named::Cyclic(_) if name.trim().to_lowercase().starts_with('z') => vec!["1".into()],
            _ => fam.standard.iter().map(|(n, _)| n.clone()).collect(),
        }
    } else {
        gens.iter().map(|g| g.trim().to_string()).collect()
    };

    // additive labelling?
    let numeric = |g: &str| g.parse::<i64>().ok();
    let tuple = |g: &str| -> Option<Vec<i64>> {
        let b = g.strip_prefix('(')?.strip_suffix(')')?;
        b.split(',').map(|x| x.trim().parse::<i64>().ok()).collect()
    };
    let additive_cyclic = matches!(named, Named::Cyclic(_)) && !names.is_empty() && names.iter().all(|g| numeric(g).is_some());
    let additive_tuple = matches!(&named, Named::Product(fs) if !names.is_empty()
        && names.iter().all(|g| tuple(g).is_some_and(|t| t.len() == fs.len())));

    // explicit spellings first; then each other single-letter name takes the
    // next standard generator not already requested (D4 <r, b>: b = s)
    let resolved: Vec<Option<Perm>> = names.iter().map(|g| resolve_gen(g, &named, &fam, &ident)).collect::<Result<_, _>>()?;
    let mut unused = fam.standard.iter().filter(|(_, p)| !resolved.iter().any(|r| r.as_ref() == Some(p)));
    let mut perms: Vec<Perm> = Vec::new();
    for (g, r) in names.iter().zip(resolved.iter()) {
        let p = match r {
            Some(p) => p.clone(),
            None => match unused.next() {
                Some((_, p)) => p.clone(),
                None => {
                    let std: Vec<&str> = fam.standard.iter().map(|(n, _)| n.as_str()).collect();
                    return Err(format!(
                        "{} has {} standard generator{} ({}); don't know what `{g}` is",
                        fam.title,
                        std.len(),
                        if std.len() == 1 { "" } else { "s" },
                        std.join(", ")
                    ));
                }
            },
        };
        perms.push(p);
    }

    let (keys, tables, words) = bfs(ident.clone(), perms.len(), |p, g| compose(p, &perms[g]))?;
    let elements: Vec<String> = if additive_cyclic {
        keys.iter().map(|p| p.first().copied().unwrap_or(0).to_string()).collect()
    } else if additive_tuple {
        let Named::Product(fs) = &named else { unreachable!() };
        keys.iter()
            .map(|p| {
                let mut base = 0usize;
                let mut parts = Vec::new();
                for &m in fs {
                    parts.push(((p[base] as usize) - base).to_string());
                    base += m;
                }
                format!("({})", parts.join(","))
            })
            .collect()
    } else if let Some(labels) = dihedral_normal_forms(&named, &fam, &names, &perms, &keys) {
        labels
    } else {
        words.iter().map(|w| word_label(w, &names)).collect()
    };
    Ok(finish(&names, tables, elements))
}

/// (FIG-POLISH) Dn drawn with the rotation r and a reflection s among its
/// generators: label every element in the normal form r^k or r^k s (her
/// course's names: r^5, not srs), with the generators' names as written.
fn dihedral_normal_forms(named: &Named, fam: &Family, names: &[String], perms: &[Perm], keys: &[Perm]) -> Option<Vec<String>> {
    let Named::Dihedral(n) = named else { return None };
    let n = *n;
    let r_std = &fam.standard.first()?.1;
    let ri = perms.iter().position(|p| p == r_std)?;
    let ident: Perm = (0..fam.degree as u32).collect();
    // a reflection: an involution that swaps the two orientation points
    let si = perms.iter().position(|p| *p != ident && compose(p, p) == ident && p[n] == n as u32 + 1)?;
    let (rn, sn) = (&names[ri], &names[si]);
    let single = rn.chars().count() == 1 && sn.chars().count() == 1;
    let juxt = |a: &str, b: &str| if single { format!("{a}{b}") } else { format!("{a} {b}") };
    let rpow = |k: usize| -> String {
        let base = if atomic(rn) { rn.clone() } else { format!("({rn})") };
        match k {
            0 => String::new(),
            1 => rn.clone(),
            k if k < 10 => format!("{base}^{k}"),
            k => format!("{base}^{{{k}}}"),
        }
    };
    let mut table: HashMap<Perm, String> = HashMap::new();
    let mut p = ident.clone();
    for k in 0..n {
        let rot = rpow(k);
        table.insert(p.clone(), if k == 0 { "e".into() } else { rot.clone() });
        table.insert(compose(&p, &perms[si]), if k == 0 { sn.clone() } else { juxt(&rot, sn) });
        p = compose(&p, &perms[ri]);
    }
    keys.iter().map(|k| table.get(k).cloned()).collect()
}

/// One requested generator name → its permutation.
/// `Ok(None)`: a plain single-letter name, to be given a standard generator.
fn resolve_gen(g: &str, named: &Named, fam: &Family, ident: &Perm) -> Result<Option<Perm>, String> {
    let title = &fam.title;
    let deg = fam.degree;
    // family-specific spellings
    match named {
        Named::Cyclic(n) => {
            if let Ok(k) = g.parse::<i64>() {
                let k = k.rem_euclid(*n as i64) as u32;
                return Ok(Some((0..*n as u32).map(|x| (x + k) % *n as u32).collect()));
            }
        }
        Named::Product(fs) => {
            if let Some(b) = g.strip_prefix('(').and_then(|r| r.strip_suffix(')')) {
                let comps: Option<Vec<i64>> = b.split(',').map(|x| x.trim().parse::<i64>().ok()).collect();
                if let Some(c) = comps {
                    if c.len() != fs.len() {
                        return Err(format!("`{g}` has {} components, but {title} has {}", c.len(), fs.len()));
                    }
                    let mut p = Vec::with_capacity(deg);
                    let mut base = 0u32;
                    for (&m, &k) in fs.iter().zip(&c) {
                        let k = k.rem_euclid(m as i64) as u32;
                        p.extend((0..m as u32).map(|x| base + (x + k) % m as u32));
                        base += m as u32;
                    }
                    return Ok(Some(p));
                }
            }
        }
        Named::Quaternion => {
            let t = g.replace('−', "-");
            let (neg, rest) = match t.strip_prefix('-') {
                Some(r) => (true, r.trim()),
                None => (false, t.as_str()),
            };
            let b = match rest {
                "1" => Some(0),
                "i" => Some(1),
                "j" => Some(2),
                "k" => Some(3),
                _ => None,
            };
            if let Some(b) = b {
                let p = quaternion_perm(b);
                let minus_one: Perm = (0..8u32).map(|x| (x + 4) % 8).collect();
                return Ok(Some(if neg { compose(&p, &minus_one) } else { p }));
            }
        }
        Named::Symmetric(n) | Named::Alternating(n) => {
            if let Some(res) = parse_cycles(g, *n, title) {
                let p = cycle_perm(*n, &res?);
                if matches!(named, Named::Alternating(_)) && !is_even(&p) {
                    return Err(format!("`{g}` is an odd permutation, so it isn't in {title}"));
                }
                return Ok(Some(p));
            }
        }
        Named::Dihedral(_) => {}
    }
    // a named letter (r, s, i, j, k)
    if let Some((_, p)) = fam.letters.iter().find(|(n, _)| n == g) {
        return Ok(Some(p.clone()));
    }
    // a word in the letters: "r^2", "rs", "r^-1"
    if !fam.letters.is_empty() {
        let names: Vec<String> = fam.letters.iter().map(|(n, _)| n.clone()).collect();
        if let Ok(rel) = parse_word(g, &names) {
            let mut p = ident.clone();
            for l in rel {
                let q = &fam.letters[l.sym].1;
                p = compose(&p, &if l.inv { invert(q) } else { q.clone() });
            }
            return Ok(Some(p));
        }
    }
    // the identity
    if g == "e" || g == "1" || g == "()" {
        return Ok(Some(ident.clone()));
    }
    // any other single-letter name: a standard generator
    if g.chars().next().is_some_and(|c| c.is_alphabetic() || c == '\\') && atomic(g) {
        return Ok(None);
    }
    let hint = match named {
        Named::Cyclic(_) => "a number like 1, or a name like a",
        Named::Dihedral(_) => "r, s, or a word in them like r^2 or rs",
        Named::Product(_) => "a tuple like (1,0), or names a, b, …",
        Named::Quaternion => "i, j, k, -1, or a word like ij",
        Named::Symmetric(_) | Named::Alternating(_) => "a permutation in cycle notation like (12) or (123), or a, b",
    };
    Err(format!("don't know the generator `{g}` of {title}: use {hint}"))
}

fn invert(p: &Perm) -> Perm {
    let mut q = vec![0; p.len()];
    for (i, &x) in p.iter().enumerate() {
        q[x as usize] = i as u32;
    }
    q
}

// ---------------------------------------------------------------------------
// words and relators

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Letter {
    sym: usize,
    inv: bool,
}

const MAX_WORD: usize = 100_000;

fn inverse_word(w: &[Letter]) -> Vec<Letter> {
    w.iter().rev().map(|l| Letter { sym: l.sym, inv: !l.inv }).collect()
}

fn free_reduce(w: Vec<Letter>) -> Vec<Letter> {
    let mut out: Vec<Letter> = Vec::with_capacity(w.len());
    for l in w {
        if let Some(last) = out.last() {
            if last.sym == l.sym && last.inv != l.inv {
                out.pop();
                continue;
            }
        }
        out.push(l);
    }
    out
}

/// Parses a word: names (longest match), juxtaposition, `^n`, `^-1`, `^{-1}`,
/// `'` or `⁻¹` (inverse), superscript digits, an upper-case letter as the
/// inverse of its lower-case generator, `(…)^n`, `[x,y]` (= x⁻¹y⁻¹xy), and
/// `1`/`e` for the identity. `*`, `·`, `.` are ignored.
fn parse_word(s: &str, names: &[String]) -> Result<Vec<Letter>, String> {
    let chars: Vec<char> = s.chars().collect();
    let mut p = WordParser { c: &chars, i: 0, names, src: s };
    let w = p.word(None)?;
    p.skip_ws();
    if p.i < chars.len() {
        return Err(format!("can't read `{}` in `{s}`", chars[p.i..].iter().collect::<String>()));
    }
    Ok(free_reduce(w))
}

struct WordParser<'a> {
    c: &'a [char],
    i: usize,
    names: &'a [String],
    src: &'a str,
}

impl WordParser<'_> {
    fn skip_ws(&mut self) {
        while self.i < self.c.len() && (self.c[self.i].is_whitespace() || matches!(self.c[self.i], '*' | '·' | '.' | '⋅')) {
            self.i += 1;
        }
    }
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }
    fn word(&mut self, close: Option<char>) -> Result<Vec<Letter>, String> {
        let mut w = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                None => break,
                Some(c) if Some(c) == close || c == ',' || c == ')' || c == ']' => break,
                _ => {}
            }
            let f = self.factor()?;
            w.extend(f);
            if w.len() > MAX_WORD {
                return Err(format!("`{}` is too long", self.src));
            }
        }
        Ok(w)
    }
    fn factor(&mut self) -> Result<Vec<Letter>, String> {
        let mut atom = self.atom()?;
        loop {
            match self.peek() {
                Some('^') => {
                    self.i += 1;
                    let k = self.exponent()?;
                    atom = power(&atom, k, self.src)?;
                }
                Some('\'') => {
                    self.i += 1;
                    atom = inverse_word(&atom);
                }
                Some('⁻') | Some('¹') | Some('²') | Some('³') | Some('⁰'..='⁹') => {
                    let neg = self.peek() == Some('⁻');
                    if neg {
                        self.i += 1;
                    }
                    let mut digits = String::new();
                    while let Some(c) = self.peek() {
                        let d = match c {
                            '⁰' => '0', '¹' => '1', '²' => '2', '³' => '3', '⁴' => '4',
                            '⁵' => '5', '⁶' => '6', '⁷' => '7', '⁸' => '8', '⁹' => '9',
                            _ => break,
                        };
                        digits.push(d);
                        self.i += 1;
                    }
                    let k: i64 = digits.parse().map_err(|_| format!("bad exponent in `{}`", self.src))?;
                    atom = power(&atom, if neg { -k } else { k }, self.src)?;
                }
                _ => break,
            }
        }
        Ok(atom)
    }
    fn exponent(&mut self) -> Result<i64, String> {
        self.skip_ws();
        let (open, close) = match self.peek() {
            Some('{') => (true, '}'),
            Some('(') => (true, ')'),
            _ => (false, ' '),
        };
        if open {
            self.i += 1;
        }
        self.skip_ws();
        let mut neg = false;
        if matches!(self.peek(), Some('-') | Some('−')) {
            neg = true;
            self.i += 1;
        }
        let start = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.i += 1;
        }
        let digits: String = self.c[start..self.i].iter().collect();
        if digits.is_empty() {
            return Err(format!("`^` needs a number in `{}`", self.src));
        }
        if open {
            self.skip_ws();
            if self.peek() != Some(close) {
                return Err(format!("unclosed exponent in `{}`", self.src));
            }
            self.i += 1;
        }
        let k: i64 = digits.parse().map_err(|_| format!("exponent too large in `{}`", self.src))?;
        if k > MAX_WORD as i64 {
            return Err(format!("exponent {k} is too large in `{}`", self.src));
        }
        Ok(if neg { -k } else { k })
    }
    fn atom(&mut self) -> Result<Vec<Letter>, String> {
        self.skip_ws();
        let Some(c) = self.peek() else { return Err(format!("`{}` ends early", self.src)) };
        if c == '(' {
            self.i += 1;
            let w = self.word(Some(')'))?;
            if self.peek() != Some(')') {
                return Err(format!("unbalanced `(` in `{}`", self.src));
            }
            self.i += 1;
            return Ok(w);
        }
        if c == '[' {
            self.i += 1;
            let x = self.word(Some(','))?;
            if self.peek() != Some(',') {
                return Err(format!("a commutator is written [x, y] in `{}`", self.src));
            }
            self.i += 1;
            let y = self.word(Some(']'))?;
            if self.peek() != Some(']') {
                return Err(format!("unbalanced `[` in `{}`", self.src));
            }
            self.i += 1;
            let mut w = inverse_word(&x);
            w.extend(inverse_word(&y));
            w.extend(x);
            w.extend(y);
            return Ok(w);
        }
        // longest generator name at this position
        let rest: String = self.c[self.i..].iter().collect();
        let best = self
            .names
            .iter()
            .enumerate()
            .filter(|(_, n)| !n.is_empty() && rest.starts_with(n.as_str()))
            .max_by_key(|(_, n)| n.len());
        if let Some((g, n)) = best {
            self.i += n.chars().count();
            return Ok(vec![Letter { sym: g, inv: false }]);
        }
        // upper case = inverse
        if c.is_uppercase() {
            let lower: String = c.to_lowercase().collect();
            if let Some(g) = self.names.iter().position(|n| *n == lower) {
                self.i += 1;
                return Ok(vec![Letter { sym: g, inv: true }]);
            }
        }
        if c == '1' || c == 'e' {
            self.i += 1;
            return Ok(vec![]);
        }
        Err(format!(
            "`{}` in `{}` isn't a generator ({})",
            rest.chars().next().unwrap_or(' '),
            self.src,
            self.names.join(", ")
        ))
    }
}

fn power(w: &[Letter], k: i64, src: &str) -> Result<Vec<Letter>, String> {
    let base = if k < 0 { inverse_word(w) } else { w.to_vec() };
    let k = k.unsigned_abs() as usize;
    if base.len().saturating_mul(k) > MAX_WORD {
        return Err(format!("`{src}` is too long"));
    }
    Ok(base.repeat(k))
}

/// A relator or a relation `x = y` (→ x y⁻¹; chains give several).
fn parse_relators(s: &str, names: &[String]) -> Result<Vec<Vec<Letter>>, String> {
    let sides: Vec<&str> = s.split('=').collect();
    let words: Vec<Vec<Letter>> = sides.iter().map(|x| parse_word(x, names)).collect::<Result<_, _>>()?;
    if words.len() == 1 {
        return Ok(words);
    }
    Ok(words
        .windows(2)
        .map(|p| {
            let mut w = p[0].clone();
            w.extend(inverse_word(&p[1]));
            free_reduce(w)
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Todd–Coxeter (HLT with coincidences, after Holt, Handbook of CGT §5.1),
// over the trivial subgroup: cosets are the group's elements.

const UNDEF: u32 = u32::MAX;

struct CosetTable {
    ncols: usize,
    table: Vec<u32>,
    /// coincidence forwarding: p[c] == c iff c is live
    p: Vec<u32>,
    live: usize,
    queue: Vec<u32>,
}

#[derive(Debug)]
enum TcError {
    TooMany,
}

impl CosetTable {
    fn new(ncols: usize) -> Self {
        CosetTable { ncols, table: vec![UNDEF; ncols], p: vec![0], live: 1, queue: Vec::new() }
    }
    fn get(&self, c: u32, x: usize) -> u32 {
        self.table[c as usize * self.ncols + x]
    }
    fn set(&mut self, c: u32, x: usize, d: u32) {
        self.table[c as usize * self.ncols + x] = d;
    }
    fn is_live(&self, c: u32) -> bool {
        self.p[c as usize] == c
    }
    fn define(&mut self, c: u32, x: usize) -> Result<(), TcError> {
        if self.live >= MAX_LIVE_COSETS || self.p.len() >= MAX_DEFINED_COSETS {
            return Err(TcError::TooMany);
        }
        let d = self.p.len() as u32;
        self.p.push(d);
        self.table.extend(std::iter::repeat_n(UNDEF, self.ncols));
        self.live += 1;
        self.set(c, x, d);
        self.set(d, x ^ 1, c);
        Ok(())
    }
    fn rep(&mut self, c: u32) -> u32 {
        let mut l = c;
        while self.p[l as usize] != l {
            l = self.p[l as usize];
        }
        // path compression
        let mut k = c;
        while self.p[k as usize] != l {
            let next = self.p[k as usize];
            self.p[k as usize] = l;
            k = next;
        }
        l
    }
    fn merge(&mut self, k: u32, l: u32) {
        let a = self.rep(k);
        let b = self.rep(l);
        if a != b {
            let (lo, hi) = if a < b { (a, b) } else { (b, a) };
            self.p[hi as usize] = lo;
            self.live -= 1;
            self.queue.push(hi);
        }
    }
    fn coincidence(&mut self, a: u32, b: u32) {
        self.queue.clear();
        self.merge(a, b);
        let mut i = 0;
        while i < self.queue.len() {
            let g = self.queue[i];
            i += 1;
            for x in 0..self.ncols {
                let d = self.get(g, x);
                if d == UNDEF {
                    continue;
                }
                self.set(d, x ^ 1, UNDEF);
                let mu = self.rep(g);
                let nu = self.rep(d);
                let mx = self.get(mu, x);
                if mx != UNDEF {
                    self.merge(nu, mx);
                } else {
                    let nx = self.get(nu, x ^ 1);
                    if nx != UNDEF {
                        self.merge(mu, nx);
                    } else {
                        self.set(mu, x, nu);
                        self.set(nu, x ^ 1, mu);
                    }
                }
            }
        }
    }
    fn scan_and_fill(&mut self, a: u32, w: &[usize]) -> Result<(), TcError> {
        let r = w.len();
        let (mut f, mut b) = (a, a);
        let (mut i, mut j) = (0usize, r); // j is exclusive: w[i..j] unscanned
        loop {
            while i < r && self.get(f, w[i]) != UNDEF {
                f = self.get(f, w[i]);
                i += 1;
            }
            if i >= r {
                if f != a {
                    self.coincidence(f, a);
                }
                return Ok(());
            }
            while j > i && self.get(b, w[j - 1] ^ 1) != UNDEF {
                b = self.get(b, w[j - 1] ^ 1);
                j -= 1;
            }
            if j < i + 1 {
                // fully scanned from both ends: f and b must agree
                self.coincidence(f, b);
                return Ok(());
            } else if j == i + 1 {
                // one gap: a deduction
                self.set(f, w[i], b);
                self.set(b, w[i] ^ 1, f);
                return Ok(());
            }
            self.define(f, w[i])?;
        }
    }
}

fn presented_group(pgens: &[String], rels: &[String], gens: &[String]) -> Result<Group, String> {
    let pnames: Vec<String> = pgens.iter().map(|g| g.trim().to_string()).collect();
    for (k, n) in pnames.iter().enumerate() {
        if n.is_empty() {
            return Err("a presentation's generator name is empty".into());
        }
        if pnames[..k].contains(n) {
            return Err(format!("the generator `{n}` is listed twice"));
        }
    }
    let mut relators: Vec<Vec<usize>> = Vec::new();
    for r in rels {
        if r.trim().is_empty() {
            continue;
        }
        for w in parse_relators(r, &pnames).map_err(|e| format!("relator `{}`: {e}", r.trim()))? {
            if !w.is_empty() {
                relators.push(w.iter().map(|l| 2 * l.sym + l.inv as usize).collect());
            }
        }
    }
    let ncols = 2 * pnames.len();
    // N cosets, table[c][col]
    let (n, table): (usize, Vec<Vec<u32>>) = if ncols == 0 {
        (1, vec![vec![]])
    } else {
        let mut t = CosetTable::new(ncols);
        let fail = |_: TcError| {
            format!("coset enumeration didn't close within {MAX_LIVE_COSETS} cosets — is the group finite (and at most {MAX_ORDER} elements)?")
        };
        let mut a: u32 = 0;
        while (a as usize) < t.p.len() {
            if t.is_live(a) {
                for w in &relators {
                    t.scan_and_fill(a, w).map_err(fail)?;
                    if !t.is_live(a) {
                        break;
                    }
                }
                if t.is_live(a) {
                    for x in 0..ncols {
                        if t.get(a, x) == UNDEF {
                            t.define(a, x).map_err(fail)?;
                        }
                    }
                }
            }
            a += 1;
        }
        // compact the live cosets
        let live: Vec<u32> = (0..t.p.len() as u32).filter(|&c| t.is_live(c)).collect();
        if live.len() > MAX_ORDER {
            return Err(format!(
                "the group has {} elements, more than {MAX_ORDER} (Cayley graphs are drawn up to {MAX_ORDER})",
                live.len()
            ));
        }
        let mut new_index = vec![u32::MAX; t.p.len()];
        for (k, &c) in live.iter().enumerate() {
            new_index[c as usize] = k as u32;
        }
        let tab = live
            .iter()
            .map(|&c| {
                (0..ncols)
                    .map(|x| {
                        let d = t.get(c, x);
                        debug_assert!(d != UNDEF && t.is_live(d));
                        new_index[d as usize]
                    })
                    .collect()
            })
            .collect();
        (live.len(), tab)
    };

    // the requested generators, as words in the presentation's generators
    let names: Vec<String> = if gens.is_empty() { pnames.clone() } else { gens.iter().map(|g| g.trim().to_string()).collect() };
    let mut perms: Vec<Vec<u32>> = Vec::new();
    for g in &names {
        let w = parse_word(g, &pnames).map_err(|e| format!("generator `{g}`: {e}"))?;
        let cols: Vec<usize> = w.iter().map(|l| 2 * l.sym + l.inv as usize).collect();
        perms.push((0..n as u32).map(|c| cols.iter().fold(c, |c, &x| table[c as usize][x])).collect());
    }
    let (_, tables, words) = bfs(0u32, perms.len(), |&c, g| perms[g][c as usize])?;
    let elements = words.iter().map(|w| word_label(w, &names)).collect();
    Ok(finish(&names, tables, elements))
}
