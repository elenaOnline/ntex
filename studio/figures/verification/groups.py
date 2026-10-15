"""Independent group computations for the ADVERSARY (no code shared with
figures/src/cayley.rs). Two derivations per group:

  1. a concrete model (dihedral pairs, quaternion units, tuples mod n,
     permutations) closed under right multiplication by the generators, and
  2. my own Todd-Coxeter (HLT with coincidences) on a presentation,

and, as a third opinion, sympy's FpGroup.order() for every presentation.
Every group is reported as: elements 0..N-1 (0 = identity), and for each
generator the right-multiplication permutation  elements[i]*g = elements[perm[i]].
"""
from itertools import permutations as _perms


# --------------------------------------------------------------------------
# words: "a^4", "(ab)^2", "a^2b^-1", "b^{-1}", gens may be multi-letter

def parse_word(w, gens):
    """-> list of (gen_index, +1|-1). Longest-match generator names."""
    w = w.replace(" ", "")
    pos = 0
    names = sorted(range(len(gens)), key=lambda i: -len(gens[i]))

    def exponent():
        nonlocal pos
        if pos < len(w) and w[pos] == "^":
            pos += 1
            brace = pos < len(w) and w[pos] == "{"
            if brace:
                pos += 1
            start = pos
            if pos < len(w) and w[pos] in "+-":
                pos += 1
            while pos < len(w) and w[pos].isdigit():
                pos += 1
            k = int(w[start:pos])
            if brace:
                assert w[pos] == "}", w
                pos += 1
            return k
        return 1

    def seq(stop=None):
        nonlocal pos
        out = []
        while pos < len(w) and w[pos] != stop:
            if w[pos] == "(":
                pos += 1
                inner = seq(")")
                assert w[pos] == ")", w
                pos += 1
                k = exponent()
                out += power(inner, k)
                continue
            for i in names:
                if w.startswith(gens[i], pos):
                    pos += len(gens[i])
                    k = exponent()
                    out += power([(i, 1)], k)
                    break
            else:
                raise ValueError(f"bad word {w!r} at {pos}")
        return out

    def power(word, k):
        if k >= 0:
            return word * k
        inv = [(g, -s) for (g, s) in reversed(word)]
        return inv * (-k)

    out = seq()
    assert pos == len(w), w
    return out


# --------------------------------------------------------------------------
# Todd-Coxeter, HLT + coincidences (Holt, Handbook of CGT, 5.1)

def todd_coxeter(ngens, rels, max_cosets=200000):
    """rels: list of words [(g, ±1)]; returns list of right-mult perms, or
    None if it exceeds max_cosets (the group is huge or infinite)."""
    ncol = 2 * ngens
    col = lambda g, s: 2 * g + (0 if s > 0 else 1)
    inv = lambda x: x ^ 1
    table = [[None] * ncol]
    p = [0]

    def rep(c):
        r = c
        while p[r] != r:
            r = p[r]
        while p[c] != r:
            p[c], c = r, p[c]
        return r

    def merge(k, l, q):
        k, l = rep(k), rep(l)
        if k == l:
            return
        m, l = min(k, l), max(k, l)
        p[l] = m
        q.append(l)

    def coincidence(a, b):
        q = []
        merge(a, b, q)
        i = 0
        while i < len(q):
            e = q[i]
            i += 1
            for x in range(ncol):
                f = table[e][x]
                if f is None:
                    continue
                if table[f][inv(x)] == e:
                    table[f][inv(x)] = None
                e1, f1 = rep(e), rep(f)
                if table[e1][x] is not None:
                    merge(f1, table[e1][x], q)
                elif table[f1][inv(x)] is not None:
                    merge(e1, table[f1][inv(x)], q)
                else:
                    table[e1][x] = f1
                    table[f1][inv(x)] = e1

    def define(c, x):
        if len(table) >= max_cosets:
            raise OverflowError
        d = len(table)
        table.append([None] * ncol)
        p.append(d)
        table[c][x] = d
        table[d][inv(x)] = c

    def scan_and_fill(c, w):
        f = b = c
        i, j = 0, len(w) - 1
        while True:
            while i <= j and table[f][w[i]] is not None:
                f = table[f][w[i]]
                i += 1
            if i > j:
                if f != c:
                    coincidence(f, c)
                return
            while j >= i and table[b][inv(w[j])] is not None:
                b = table[b][inv(w[j])]
                j -= 1
            if j < i:
                coincidence(f, b)
                return
            if i == j:
                table[f][w[i]] = b
                table[b][inv(w[i])] = f
                return
            define(f, w[i])

    cw = [[col(g, s) for (g, s) in r] for r in rels]
    try:
        c = 0
        while c < len(table):
            if p[c] == c:
                for r in cw:
                    if p[c] != c:
                        break
                    scan_and_fill(c, r)
                if p[c] == c:
                    for x in range(ncol):
                        if table[c][x] is None:
                            define(c, x)
            c += 1
    except OverflowError:
        return None
    live = [c for c in range(len(table)) if p[c] == c]
    idx = {c: i for i, c in enumerate(live)}
    return [[idx[rep(table[c][2 * g])] for c in live] for g in range(ngens)]


# --------------------------------------------------------------------------
# concrete models: closure under right multiplication

def closure(identity, gens, mul):
    elems = [identity]
    index = {identity: 0}
    i = 0
    while i < len(elems):
        for g in gens:
            h = mul(elems[i], g)
            if h not in index:
                index[h] = len(elems)
                elems.append(h)
        i += 1
    perms = [[index[mul(x, g)] for x in elems] for g in gens]
    return elems, perms


def dihedral(n):  # (k, f) = r^k s^f
    mul = lambda a, b: ((a[0] + (-1) ** a[1] * b[0]) % n, a[1] ^ b[1])
    return closure((0, 0), [(1, 0), (0, 1)], mul)


def cyclic_additive(n, gens):
    return closure(0, gens, lambda a, b: (a + b) % n)


def product(ms, gens):
    return closure(tuple(0 for _ in ms), gens,
                   lambda a, b: tuple((x + y) % m for x, y, m in zip(a, b, ms)))


# quaternion units: (sign, u) with u in "1ijk"
_QT = {("1", x): (1, x) for x in "1ijk"}
_QT.update({(x, "1"): (1, x) for x in "1ijk"})
_QT.update({("i", "i"): (-1, "1"), ("j", "j"): (-1, "1"), ("k", "k"): (-1, "1"),
            ("i", "j"): (1, "k"), ("j", "k"): (1, "i"), ("k", "i"): (1, "j"),
            ("j", "i"): (-1, "k"), ("k", "j"): (-1, "i"), ("i", "k"): (-1, "j")})


def q8():
    def mul(a, b):
        s, u = _QT[(a[1], b[1])]
        return (a[0] * b[0] * s, u)
    return closure((1, "1"), [(1, "i"), (1, "j")], mul)


def perm_mul(a, b):  # right action: x -> b(a(x)), i.e. "a then b"
    return tuple(b[a[i]] for i in range(len(a)))


def sym(n, gens):
    return closure(tuple(range(n)), gens, perm_mul)


# --------------------------------------------------------------------------
# invariants

def perm_order(perm):
    n, cur, k = len(perm), list(perm), 1
    ident = list(range(n))
    while cur != ident:
        cur = [perm[c] for c in cur]
        k += 1
    return k


def element_orders(perms):
    """Orders of the group elements, from the right-regular action: element i
    corresponds to the permutation 'left multiply by elements[i]', but orders
    equal those of the right-regular images, which we get by walking words."""
    n = len(perms[0])
    # build each element's right-regular permutation by BFS on words
    ident = tuple(range(n))
    reg = {0: ident}
    queue = [0]
    while queue:
        i = queue.pop(0)
        for g in perms:
            j = g[i]
            if j not in reg:
                # rho(elements[i]*g) = rho(elements[i]) then rho(g)
                reg[j] = tuple(g[reg[i][x]] for x in range(n))
                queue.append(j)
    counts = {}
    for i in range(n):
        o = perm_order(reg[i])
        counts[o] = counts.get(o, 0) + 1
    return dict(sorted(counts.items())), reg


def abelian(perms):
    n = len(perms[0])
    return all(perm_mul(tuple(a), tuple(b)) == perm_mul(tuple(b), tuple(a))
               for a in perms for b in perms)


def edges_for(order, size):
    """Edges drawn for one generator: |G| arcs, or |G|/2 undirected edges for
    an involution (FIGURES.md 'An involution's edge is drawn once')."""
    if order == 1:
        return 0  # acts trivially: degenerate; checked separately
    return size // 2 if order == 2 else size


def report(gen_names, perms, source):
    size = len(perms[0])
    orders, _ = element_orders(perms)
    gens = []
    for name, pm in zip(gen_names, perms):
        o = perm_order(pm)
        gens.append({"name": name, "order": o, "edges": edges_for(o, size), "perm": list(pm)})
    return {"order": size, "gens": gens, "element_orders": {str(k): v for k, v in orders.items()},
            "abelian": abelian(perms), "total_edges": sum(g["edges"] for g in gens),
            "source": source}


def sympy_order(gens, rels):
    from sympy.combinatorics.free_groups import free_group
    from sympy.combinatorics.fp_groups import FpGroup
    F, *xs = free_group(",".join(f"x{i}" for i in range(len(gens))))
    def to_elem(word):
        e = F.identity
        for g, s in word:
            e = e * (xs[g] if s > 0 else xs[g] ** -1)
        return e
    G = FpGroup(F, [to_elem(parse_word(r, gens)) for r in rels])
    return G.order()


def presentation(gens, rels, check_sympy=True):
    words = [parse_word(r, gens) for r in rels]
    perms = todd_coxeter(len(gens), words, max_cosets=20000)
    if perms is None:
        return {"order": None, "infinite_or_huge": True, "source": "todd-coxeter overflowed 20000 cosets"}
    rep = report(gens, perms, "own Todd-Coxeter")
    # relations hold: each relator, read from every element, returns to it
    for w in words:
        for i in range(rep["order"]):
            c = i
            for g, s in w:
                c = perms[g][c] if s > 0 else perms[g].index(c)
            assert c == i, ("relator fails", gens, rels)
    if check_sympy:
        o = sympy_order(gens, rels)
        assert o == rep["order"], (gens, rels, o, rep["order"])
        rep["source"] += f"; sympy FpGroup.order() = {o} agrees"
    return rep


def named(name, gens):
    """The groups FIGURES.md lists, with my reading of the generators."""
    import re
    key = name.replace(" ", "")
    m = re.fullmatch(r"D(\d+)", key)
    if m:
        n = int(m.group(1))
        _, perms = dihedral(n)
        # by name: r = rotation, s = reflection; otherwise first = rotation
        if gens == ["s", "r"]:
            perms = [perms[1], perms[0]]
        return report(gens, perms, f"dihedral model r^k s^f (r = rotation, s = reflection, by name), order 2n = {2*n}")
    m = re.fullmatch(r"[CZ](\d+)", key)
    if m:
        n = int(m.group(1))
        if all(g.lstrip("-").isdigit() for g in gens):
            _, perms = cyclic_additive(n, [int(g) for g in gens])
            return report(gens, perms, f"Z/{n} additive, generators as integers")
        _, perms = cyclic_additive(n, [1] * len(gens))
        return report(gens, perms, f"cyclic of order {n}, the generator is a fixed generator")
    m = re.fullmatch(r"Z(\d+)xZ(\d+)", "Z2xZ2" if key == "V4" else key)
    if m:
        a, b = int(m.group(1)), int(m.group(2))
        _, perms = product((a, b), [(1, 0), (0, 1)][: len(gens)])
        return report(gens, perms, f"Z{a} x Z{b} as pairs, a=(1,0), b=(0,1)")
    if key == "Q8":
        _, perms = q8()
        return report(gens, perms, "quaternion units, generators i, j")
    if key == "S3":
        _, perms = sym(3, [(1, 0, 2), (1, 2, 0)])
        return report(gens, perms, "S3 as permutations; generator choice is the implementation's (order and element orders checked)")
    if key == "S4":
        _, perms = sym(4, [(1, 0, 2, 3), (1, 2, 3, 0)])
        return report(gens, perms, "S4 as permutations (transposition, 4-cycle); generator choice is the implementation's")
    if key == "A4":
        _, perms = sym(4, [(1, 0, 3, 2), (1, 2, 0, 3)])
        return report(gens, perms, "A4 as even permutations ((12)(34), (123)); generator choice is the implementation's")
    raise KeyError(name)


if __name__ == "__main__":
    # self-checks: concrete models against presentations
    assert presentation(["r", "s"], ["r^4", "s^2", "(rs)^2"])["order"] == 8
    d4 = named("D4", ["r", "s"])
    assert d4["element_orders"] == {"1": 1, "2": 5, "4": 2}
    assert named("Q8", ["i", "j"])["element_orders"] == {"1": 1, "2": 1, "4": 6}
    print(presentation(["a", "b"], ["a^2", "b^3", "(ab)^5"])["order"])
    print(presentation(["a", "b"], ["a^4", "a^2b^2", "b^3aba"]))
    print(presentation(["a", "b"], ["a^2", "b^2"]))
