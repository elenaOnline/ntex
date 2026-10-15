//! FIG-CORE-B: cayley::group — named groups, Todd–Coxeter, shortlex words.
use ntex_figures::cayley::{self, Group, Kind};
use ntex_figures::model::GroupSpec;
use std::collections::HashSet;
use std::time::{Duration, Instant};

fn named(name: &str, gens: &[&str]) -> Result<Group, String> {
    let gens: Vec<String> = gens.iter().map(|s| s.to_string()).collect();
    cayley::group(&GroupSpec::Named(name.into()), &gens)
}

fn pres(gens: &[&str], rels: &[&str], use_gens: &[&str]) -> Result<Group, String> {
    let g: Vec<String> = gens.iter().map(|s| s.to_string()).collect();
    let r: Vec<String> = rels.iter().map(|s| s.to_string()).collect();
    let u: Vec<String> = use_gens.iter().map(|s| s.to_string()).collect();
    cayley::group(&GroupSpec::Presentation(g, r), &u)
}

fn gi(g: &Group, name: &str) -> usize {
    g.gens.iter().position(|(n, _)| n == name).unwrap_or_else(|| panic!("no generator {name}"))
}

/// Element reached from `from` by right-multiplying the generators (by index)
/// in `letters`; negative entries (−k−1) mean the inverse of generator k.
fn walk(g: &Group, from: usize, letters: &[i64]) -> usize {
    letters.iter().fold(from, |x, &l| {
        if l >= 0 {
            g.gens[l as usize].1[x]
        } else {
            let t = &g.gens[(-l - 1) as usize].1;
            t.iter().position(|&y| y == x).unwrap()
        }
    })
}

/// A relation holds at every element (right multiplication: x·w = x).
fn holds(g: &Group, letters: &[i64]) -> bool {
    (0..g.order()).all(|x| walk(g, x, letters) == x)
}

fn power(l: i64, k: usize) -> Vec<i64> {
    vec![l; k]
}

/// Parses a single-character-generator label ("r^2s", "r^{10}") into
/// generator indices.
fn letters_of(word: &str, names: &[String]) -> Vec<usize> {
    if word == "e" {
        return vec![];
    }
    let cs: Vec<char> = word.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        let c: String = if cs[i] == '(' {
            let end = i + cs[i..].iter().position(|&c| c == ')').unwrap();
            cs[i..=end].iter().collect()
        } else {
            cs[i].to_string()
        };
        let g = names.iter().position(|n| *n == c).unwrap_or_else(|| panic!("`{c}` in `{word}`"));
        i += c.chars().count();
        let mut k = 1;
        if i < cs.len() && cs[i] == '^' {
            i += 1;
            let braced = cs[i] == '{';
            if braced {
                i += 1;
            }
            let s = i;
            while i < cs.len() && cs[i].is_ascii_digit() {
                i += 1;
            }
            k = cs[s..i].iter().collect::<String>().parse().unwrap();
            if braced {
                assert_eq!(cs[i], '}');
                i += 1;
            }
            assert!(k >= 2, "power 1 written out in `{word}`");
        }
        out.extend(std::iter::repeat_n(g, k));
    }
    out
}

/// The structural checks every group must pass.
fn check_structure(g: &Group) {
    let n = g.order();
    assert!(n >= 1);
    let set: HashSet<&String> = g.elements.iter().collect();
    assert_eq!(set.len(), n, "words are unique: {:?}", g.elements);
    for (name, t) in &g.gens {
        assert_eq!(t.len(), n, "table of {name}");
        let mut seen = vec![false; n];
        for &j in t {
            assert!(j < n);
            assert!(!seen[j], "table of {name} is a permutation");
            seen[j] = true;
        }
    }
}

/// Words are shortlex (length, then generator order), non-repeating runs are
/// collapsed, and each word re-evaluates to its own index.
fn check_words(g: &Group) {
    check_structure(g);
    assert_eq!(g.elements[0], "e");
    let names: Vec<String> = g.gens.iter().map(|(n, _)| n.clone()).collect();
    let ws: Vec<Vec<usize>> = g.elements.iter().map(|w| letters_of(w, &names)).collect();
    for (i, w) in ws.iter().enumerate() {
        let at = w.iter().fold(0, |x, &l| g.gens[l].1[x]);
        assert_eq!(at, i, "word `{}` evaluates to its own element", g.elements[i]);
        // no run written as separate letters, e.g. "rr"
        assert!(!g.elements[i].chars().collect::<Vec<_>>().windows(2).any(|p| p[0] == p[1] && p[0].is_alphabetic()),
            "runs collapse: `{}`", g.elements[i]);
    }
    for p in ws.windows(2) {
        assert!((p[0].len(), &p[0]) < (p[1].len(), &p[1]), "shortlex: {:?} before {:?}", p[0], p[1]);
    }
    // shortest: no element is reachable by a shorter word than its own (BFS check)
    let n = g.order();
    let mut dist = vec![usize::MAX; n];
    dist[0] = 0;
    let mut q = std::collections::VecDeque::from([0usize]);
    while let Some(x) = q.pop_front() {
        for (_, t) in &g.gens {
            if dist[t[x]] == usize::MAX {
                dist[t[x]] = dist[x] + 1;
                q.push_back(t[x]);
            }
        }
    }
    for i in 0..n {
        assert_eq!(dist[i], ws[i].len(), "`{}` is a shortest word", g.elements[i]);
    }
}

fn check_dihedral(n: usize) {
    let g = named(&format!("D{n}"), &["r", "s"]).unwrap();
    // (FIG-POLISH) Dn's vertices are named in the normal form r^k, r^k s
    let rk = |k: usize| match k { 0 => String::new(), 1 => "r".to_string(), k if k < 10 => format!("r^{k}"), k => format!("r^{{{k}}}") };
    let mut want: Vec<String> = (0..n).map(|k| if k == 0 { "e".into() } else { rk(k) }).collect();
    want.extend((0..n).map(|k| format!("{}s", rk(k))));
    let mut got = g.elements.clone();
    got.sort();
    want.sort();
    assert_eq!(got, want, "D{n} normal forms");
    assert_eq!(g.elements[0], "e");
    assert_eq!(g.order(), 2 * n);
    let (r, s) = (gi(&g, "r") as i64, gi(&g, "s") as i64);
    assert!(holds(&g, &power(r, n)));
    assert!(!holds(&g, &power(r, n - 1)) || n == 1);
    assert!(holds(&g, &[s, s]));
    assert!(holds(&g, &[r, s, r, s]));
    // s r s = r^-1
    assert!(holds(&g, &[s, r, s, r]));
    assert!(g.is_involution(s as usize));
}

#[test]
fn dihedral_groups() {
    for n in [3, 4, 5, 6] {
        check_dihedral(n);
    }
    let d4 = named("D4", &["r", "s"]).unwrap();
    assert!(d4.elements.contains(&"r^2".to_string()));
    assert!(d4.elements.contains(&"r^3".to_string()));
    assert_eq!(&d4.elements[..6], &["e", "r", "s", "r^2", "rs", "r^3s"]); // sr = r^3s
    assert!(!d4.is_abelian());
}

#[test]
fn dihedral_small_and_aliases() {
    assert_eq!(named("D1", &["r", "s"]).unwrap().order(), 2);
    assert_eq!(named("D2", &["r", "s"]).unwrap().order(), 4);
    // a, b → the standard r, s; empty → r, s
    let g = named("d 4", &["a", "b"]).unwrap();
    assert_eq!(g.order(), 8);
    assert!(g.elements.contains(&"a^3".to_string()));
    // a leftover letter takes the standard generator not already named
    let g = named("D4", &["r", "b"]).unwrap();
    assert_eq!(g.order(), 8);
    assert!(holds(&g, &[1, 1]));
    let g = named("D5", &[]).unwrap();
    assert_eq!(g.order(), 10);
    assert_eq!(g.gens[0].0, "r");
    // a word as a generator: <r^2, s> is a Klein four subgroup of D4
    let g = named("D4", &["r^2", "s"]).unwrap();
    assert_eq!(g.order(), 4);
    check_structure(&g);
    assert!(g.is_abelian());
}

#[test]
fn quaternions() {
    let g = named("Q8", &["i", "j"]).unwrap();
    check_words(&g);
    assert_eq!(g.order(), 8);
    let (i, j) = (gi(&g, "i") as i64, gi(&g, "j") as i64);
    assert!(holds(&g, &power(i, 4)));
    assert!(!holds(&g, &power(i, 2)));
    // i^2 = j^2 = (ij)^2
    let i2 = walk(&g, 0, &[i, i]);
    assert_eq!(walk(&g, 0, &[j, j]), i2);
    assert_eq!(walk(&g, 0, &[i, j, i, j]), i2);
    // j i j^-1 = i^-1
    assert!(holds(&g, &[j, i, -j - 1, i]));
    // i, j, k, -1 all accepted
    let g = named("q8", &["i", "j", "k", "-1"]).unwrap();
    assert_eq!(g.order(), 8);
    check_structure(&g);
    let (i, j, k, m) = (0i64, 1, 2, 3);
    assert_eq!(walk(&g, 0, &[i, j]), walk(&g, 0, &[k]));
    assert_eq!(walk(&g, 0, &[i, i]), walk(&g, 0, &[m]));
    assert!(g.is_involution(3));
    // <i> alone: the cyclic subgroup of order 4
    assert_eq!(named("Q8", &["i"]).unwrap().order(), 4);
}

#[test]
fn symmetric_and_alternating() {
    let s3 = named("S3", &["(12)", "(123)"]).unwrap();
    check_words(&s3);
    assert_eq!(s3.order(), 6);
    assert!(!s3.is_abelian());
    let s3 = named("S3", &["a", "b"]).unwrap();
    check_words(&s3);
    assert_eq!(s3.order(), 6);
    let (a, b) = (0i64, 1);
    assert!(holds(&s3, &power(a, 3)));
    assert!(holds(&s3, &[b, b]));
    assert!(holds(&s3, &[a, b, a, b]));

    let s4 = named("S4", &["(1234)", "(12)"]).unwrap();
    check_structure(&s4);
    assert_eq!(s4.order(), 24);
    assert_eq!(named("S4", &["a", "b"]).unwrap().order(), 24);
    assert_eq!(named("S4", &["(1 2 3 4)", "(1,2)"]).unwrap().order(), 24);
    // a subgroup: <(12)(34), (13)(24)> = V4
    assert_eq!(named("S4", &["(12)(34)", "(13)(24)"]).unwrap().order(), 4);

    let a4 = named("A4", &["a", "b"]).unwrap();
    check_words(&a4);
    assert_eq!(a4.order(), 12);
    assert!(holds(&a4, &power(0, 3)));
    assert!(holds(&a4, &[1, 1]));
    assert!(holds(&a4, &[0, 1, 0, 1, 0, 1]));
    assert_eq!(named("A4", &["(123)", "(12)(34)"]).unwrap().order(), 12);
    assert_eq!(named("A4", &["(123)", "(234)"]).unwrap().order(), 12);
    let err = named("A4", &["(12)"]).err().unwrap();
    assert!(err.contains("odd"), "{err}");
    let err = named("S3", &["(14)"]).err().unwrap();
    assert!(err.contains("1..3"), "{err}");
    // S5 is too large for a Cayley graph; A5 (60) is fine
    assert!(named("S5", &["a", "b"]).err().unwrap().contains("96"));
    assert_eq!(named("A5", &["a", "b"]).unwrap().order(), 60);
}

#[test]
fn products_and_cyclic() {
    let g = named("Z2xZ3", &["a", "b"]).unwrap();
    check_words(&g);
    assert_eq!(g.order(), 6);
    assert!(g.is_abelian());
    assert!(holds(&g, &[0, 1, -1, -2]));
    assert_eq!(named("Z2 x Z3", &["a", "b"]).unwrap().order(), 6);
    assert_eq!(named("ℤ2×ℤ3", &["a", "b"]).unwrap().order(), 6);
    assert_eq!(named("Z2xZ2xZ2", &["a", "b", "c"]).unwrap().order(), 8);
    assert_eq!(cayley::kind(&GroupSpec::Named("Z2xZ2xZ2".into())), Kind::Product(vec![2, 2, 2]));
    for v in ["V4", "Z2xZ2", "klein"] {
        let g = named(v, &["a", "b"]).unwrap();
        check_words(&g);
        assert_eq!(g.order(), 4);
        assert!(g.is_involution(0) && g.is_involution(1));
    }
    // tuple generators → tuple labels
    let g = named("Z2xZ3", &["(1,0)", "(0,1)"]).unwrap();
    check_structure(&g);
    assert_eq!(g.elements[0], "(0,0)");
    assert!(g.elements.contains(&"(1,2)".to_string()));

    // Z6 <1>: additive labels
    let g = named("Z6", &["1"]).unwrap();
    check_structure(&g);
    assert_eq!(g.elements, ["0", "1", "2", "3", "4", "5"]);
    assert!(holds(&g, &power(0, 6)));
    assert_eq!(named("Z6", &["2"]).unwrap().elements, ["0", "2", "4"]);
    assert_eq!(named("Z6", &["2", "3"]).unwrap().order(), 6);
    // multiplicative names
    let g = named("C6", &["a"]).unwrap();
    check_words(&g);
    assert_eq!(g.elements, ["e", "a", "a^2", "a^3", "a^4", "a^5"]);
    assert_eq!(cayley::kind(&GroupSpec::Named("C6".into())), Kind::Cyclic(6));
    assert_eq!(cayley::kind(&GroupSpec::Named("D4".into())), Kind::Dihedral(4));
    // braced exponents from 10 on
    let g = named("Z12", &["g"]).unwrap();
    check_words(&g);
    assert!(g.elements.contains(&"g^{11}".to_string()));
    // too big
    assert!(named("Z97", &["1"]).err().unwrap().contains("96"));
}

#[test]
fn unknown_names_are_clear_errors() {
    let e = named("Foo7", &["a"]).err().unwrap();
    assert!(e.contains("unknown group `Foo7`") && e.contains("Dn"), "{e}");
    let e = named("D4", &["r", "s", "t"]).err().unwrap();
    assert!(e.contains("`t`"), "{e}");
    assert!(named("Z2xQ8", &["a"]).is_err());
}

#[test]
fn presentation_d4() {
    let g = pres(&["a", "b"], &["a^4", "b^2", "(ab)^2"], &["a", "b"]).unwrap();
    check_words(&g);
    assert_eq!(g.order(), 8);
    assert!(holds(&g, &power(0, 4)));
    assert!(holds(&g, &[1, 1]));
    assert!(holds(&g, &[0, 1, 0, 1]));
    assert!(!g.is_abelian());
    // a presentation keeps shortlex words (only named Dn uses r^k s)
    assert_eq!(g.elements, ["e", "a", "b", "a^2", "ab", "ba", "a^3", "a^2b"]);
}

#[test]
fn presentation_s3_and_syntax() {
    let g = pres(&["a", "b"], &["a^3", "b^2", "abab"], &["a", "b"]).unwrap();
    check_words(&g);
    assert_eq!(g.order(), 6);
    // relation syntax, inverses, commutators
    let g = pres(&["a", "b"], &["a^3 = 1", "b^2 = e", "b a b^-1 = a^{-1}"], &[]).unwrap();
    assert_eq!(g.order(), 6);
    let g = pres(&["a", "b"], &["a^4", "b^2", "bab' = A"], &["a", "b"]).unwrap();
    assert_eq!(g.order(), 8);
    let g = pres(&["x", "y"], &["x^2", "y^3", "[x,y]"], &["x", "y"]).unwrap();
    assert_eq!(g.order(), 6);
    assert!(g.is_abelian());
    // Q8 as a presentation
    let g = pres(&["i", "j"], &["i^4", "i^2 = j^2", "j i j^-1 = i^-1"], &["i", "j"]).unwrap();
    check_words(&g);
    assert_eq!(g.order(), 8);
    // a generator given as a word in the presentation
    let g = pres(&["a", "b"], &["a^4", "b^2", "(ab)^2"], &["a^2", "b"]).unwrap();
    assert_eq!(g.order(), 4);
    // cyclic, trivial
    assert_eq!(pres(&["a"], &["a^5"], &["a"]).unwrap().order(), 5);
    assert_eq!(pres(&["a"], &["a"], &["a"]).unwrap().order(), 1);
    // order 96 is allowed: Z96
    assert_eq!(pres(&["a"], &["a^96"], &["a"]).unwrap().order(), 96);
    // bad syntax is an error, not a panic
    assert!(pres(&["a", "b"], &["a^"], &["a"]).is_err());
    assert!(pres(&["a", "b"], &["c^2"], &["a"]).is_err());
    assert!(pres(&["a", "b"], &["a^2"], &["c"]).is_err());
}

#[test]
fn presentation_harder_groups() {
    // (2,3,3) triangle group = A4, (2,3,4) = S4, binary dihedral, Z2xZ2xZ2
    assert_eq!(pres(&["a", "b"], &["a^2", "b^3", "(ab)^3"], &[]).unwrap().order(), 12);
    assert_eq!(pres(&["a", "b"], &["a^2", "b^3", "(ab)^4"], &[]).unwrap().order(), 24);
    assert_eq!(pres(&["a", "b", "c"], &["a^2", "b^2", "c^2", "[a,b]", "[a,c]", "[b,c]"], &[]).unwrap().order(), 8);
    // a Coxeter group of order 48 (B3)
    assert_eq!(
        pres(&["a", "b", "c"], &["a^2", "b^2", "c^2", "(ab)^3", "(bc)^4", "(ac)^2"], &[]).unwrap().order(),
        48
    );
}

#[test]
fn infinite_or_large_presentations_fail_fast() {
    for (gens, rels) in [
        (vec!["a", "b"], vec!["a^2"]),
        (vec!["a"], vec!["a^100"]),
        (vec!["a", "b"], vec![]),
        (vec!["a", "b"], vec!["a^2", "b^3"]),
        // A5 (60) is fine, but (2,3,7) is infinite
        (vec!["a", "b"], vec!["a^2", "b^3", "(ab)^7"]),
    ] {
        let t = Instant::now();
        let r = pres(&gens, &rels, &["a"]);
        let e = r.err().unwrap_or_else(|| panic!("{rels:?} should fail"));
        assert!(e.contains("96") || e.contains("finite"), "{e}");
        assert!(t.elapsed() < Duration::from_secs(1), "{rels:?} took {:?}", t.elapsed());
    }
    let e = pres(&["a"], &["a^100"], &["a"]).err().unwrap();
    assert!(e.contains("more than 96"), "{e}");
    assert_eq!(pres(&["a", "b"], &["a^2", "b^3", "(ab)^5"], &[]).unwrap().order(), 60);
}
