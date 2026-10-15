"""The adversary's cases: drafts, the Lua I expect codegen to write, and how to type them.

Run:  python3 -I cases.py emit > cases.json   (driver input)
The draft dicts are serde JSON of ntex_studio_core::model::Draft.
"""
import json, sys

def T(s): return {"Text": s}
def Tab(n, d=""): return {"Tab": {"n": n, "default": d}}
EXIT = "Exit"
SEL = "Selection"
def Cap(k): return {"Capture": k}

def draft(trigger, output, kind="Plain", context="Math", fires="Auto", **kw):
    d = {"id": None, "trigger": trigger, "kind": kind, "context": context, "fires": fires,
         "math_where": "Any", "line": "Anywhere", "ft": "Both", "priority": 0, "desc": "", "eg": "",
         "check": "", "origin": "yours", "output": output, "raw_lua": "", "pinned": []}
    d.update(kw)
    return d

I = lambda s: "$" + s + "$"
D = lambda s: "\\[\n" + s + "\n\\]"

# name, draft, expected entry (exact), alternatives (also accepted), typings
# typing: (ft, text, [keys...], select_len or None)
CASES = []
def case(cid, title, d, entry, typings, alts=(), note=""):
    CASES.append(dict(id=cid, title=title, draft=d, entry=entry, alts=list(alts), typings=typings, note=note))

case("C01", "plain maths auto, two tab stops",
     draft("ff", [T("\\frac{"), Tab(1), T("}{"), Tab(2), T("}")]),
     '  math("ff", [[\\frac{<>}{<>}]], "yours"),',
     [("tex", I("‸"), ["ff", "a<Tab>", "b<Tab>"], None)])
case("C02", "]] inside the body needs a level-1 long string",
     draft("dbr", [T("a]]b")]),
     '  math("dbr", [=[a]]b]=], "yours"),',
     [("tex", I("‸"), ["dbr"], None)])
case("C03", "body ENDING in ] (no ]] inside) still cannot use [[…]]",
     draft("lrs", [T("\\left[ "), Tab(1), T(" \\right]")]),
     '  math("lrs", [=[\\left[ <> \\right]]=], "yours"),',
     [("tex", I("‸"), ["lrs", "x<Tab>"], None)],
     alts=['  math("lrs", "\\\\left[ <> \\\\right]", "yours"),'])
case("C04", "leading newline (Lua drops the first newline of a long string)",
     draft("nlx", [T("\n\\quad")]),
     '  math("nlx", [[\n\n\\quad]], "yours"),',
     [("tex", D("a ‸"), ["nlx"], None)])
case("C05", "trailing space kept in the Lua; dropped by ntex only in inline maths",
     draft("tsp", [T("\\otimes ")]),
     '  math("tsp", [[\\otimes ]], "yours"),',
     [("tex", I("a ‸"), ["tsp"], None), ("tex", D("a ‸"), ["tsp"], None)])
case("C06", "tab character, text, Tab, line start (a Castel-style environment)",
     draft("thm", [T("\\begin{theorem}\n\t"), Tab(1), T("\n\\end{theorem}")], context="Text", fires="Tab", line="Start"),
     '  text("thm", [[\\begin{theorem}\n\t<>\n\\end{theorem}]], "yours", { line = "start", tab = true }),',
     [("tex", "‸", ["thm<Tab>", "x<Tab>"], None), ("tex", "Say thm‸", ["<Tab>"], None)],
     alts=['  text("thm", [[\\begin{theorem}\n\t<>\n\\end{theorem}]], "yours", { tab = true, line = "start" }),',
           '  text("thm", [[\n\\begin{theorem}\n\t<>\n\\end{theorem}]], "yours", { tab = true, line = "start" }),',
           '  text("thm", [[\n\\begin{theorem}\n\t<>\n\\end{theorem}]], "yours", { line = "start", tab = true }),'],
     note="ARCHITECTURE lists opts order where, line, tab, … but its own example (and her files) write { tab = true, line = \"start\" }: both accepted")
case("C07", "literal % → %%",
     draft("pct", [T("50\\%")]),
     '  math("pct", [[50\\%%]], "yours"),',
     [("tex", I("‸"), ["pct"], None)])
case("C08", "literal %1 and %V text in a plain snippet → %%1, %%V",
     draft("pcx", [T("\\%1 \\%V")]),
     '  math("pcx", [[\\%%1 \\%%V]], "yours"),',
     [("tex", I("‸"), ["pcx"], None)])
case("C09", "literal <> → %<%> (BRIDGE's escape)",
     draft("ltg", [T("a<>b")]),
     '  math("ltg", [[a%<%>b]], "yours"),',
     [("tex", I("‸"), ["ltg"], None)])
case("C10", "literal <3> → %<3%>",
     draft("lt3", [T("x<3>y")]),
     '  math("lt3", [[x%<3%>y]], "yours"),',
     [("tex", I("‸"), ["lt3"], None)])
case("C11", "literal <:x> → %<:x%>",
     draft("ltx", [T("u<:x>v")]),
     '  math("ltx", [[u%<:x%>v]], "yours"),',
     [("tex", I("‸"), ["ltx"], None)])
case("C12", "literal <b> does not read as a tab stop: left alone",
     draft("ltb", [T("a<b>c")]),
     '  math("ltb", [[a<b>c]], "yours"),',
     [("tex", I("‸"), ["ltb"], None)],
     alts=['  math("ltb", [[a%<b%>c]], "yours"),'])
case("C13", "mirror: tab stop 1 typed twice → numbered",
     draft("bnv", [T("\\begin{"), Tab(1), T("}\n"), Tab(2), T("\n\\end{"), Tab(1), T("}")]),
     '  math("bnv", [[\\begin{<1>}\n<2>\n\\end{<1>}]], "yours"),',
     [("tex", D("‸"), ["bnv", "foo", "<Tab>", "z<Tab>"], None)],
     alts=['  math("bnv", [[\n\\begin{<1>}\n<2>\n\\end{<1>}]], "yours"),'],
     note="her files open a multi-line body with [[ and a newline (Lua drops it): accepted")
case("C14", "defaults with spaces",
     draft("dsp", [T("\\sum_{"), Tab(1, "i = 1"), T("}^{"), Tab(2, "n"), T("}")]),
     '  math("dsp", [[\\sum_{<:i = 1>}^{<:n>}]], "yours"),',
     [("tex", I("‸"), ["dsp", "<Tab>", "<Tab>"], None)])
case("C15", "out-of-order tab stops → every one numbered",
     draft("frr", [T("\\frac{"), Tab(2), T("}{"), Tab(1), T("}")]),
     '  math("frr", [[\\frac{<2>}{<1>}]], "yours"),',
     [("tex", I("‸"), ["frr", "a<Tab>", "b<Tab>"], None)])
case("C16", "exit point <0> before the last tab stop",
     draft("itd", [T("\\int "), EXIT, T(" \\, d"), Tab(1, "x")]),
     '  math("itd", [[\\int <0> \\, d<:x>]], "yours"),',
     [("tex", I("‸"), ["itd", "t<Tab>"], None)],
     alts=['  math("itd", [[\\int <0> \\, d<1:x>]], "yours"),'])
case("C17", "post: %1 is the letter before",
     draft("bbm", [T("\\mathbb{"), Cap(1), T("}")], kind="Post"),
     '  post("bbm", [[\\mathbb{%1}]], "yours"),',
     [("tex", I("‸"), ["Rbbm"], None), ("tex", I("‸"), ["bbm"], None)])
case("C18", "pattern with two captures, eg given",
     draft("(%a)vv(%d)", [T("\\vec{"), Cap(1), T("}_{"), Cap(2), T("}")], kind="Pattern", eg="xvv2"),
     '  pat("(%a)vv(%d)", [[\\vec{%1}_{%2}]], "yours", { eg = "xvv2" }),',
     [("tex", I("‸"), ["xvv2"], None)])
case("C19", "visual: %V, select then key",
     draft("D", [T("\\boldsymbol{"), SEL, T("}")], kind="Visual"),
     '  vis("D", [[\\boldsymbol{%V}]], "yours"),',
     [("tex", I("‸a+b"), ["D"], 3)])
case("C20", "text, list item rule, auto",
     draft("todo", [T("\\textbf{TODO}: ")], context="Text", line="ListItem"),
     '  text("todo", [[\\textbf{TODO}: ]], "yours", { line = "list item" }),',
     [("md", "- ‸", ["todo"], None), ("md", "‸", ["todo"], None)])
case("C21", "ft = tex only (Tab, line start)",
     draft("sect", [T("\\section{"), Tab(1), T("}")], context="Text", fires="Tab", line="Start", ft="Tex"),
     '  text("sect", [[\\section{<>}]], "yours", { line = "start", tab = true, ft = "tex" }),',
     [("tex", "‸", ["sect<Tab>"], None), ("md", "‸", ["sect<Tab>"], None)],
     alts=['  text("sect", [[\\section{<>}]], "yours", { tab = true, line = "start", ft = "tex" }),'])
case("C22", "priority 1 beats her existing trigger (sq in operations.lua)",
     draft("sq", [T("\\sqrt[3]{"), Tab(1), T("}")], priority=1),
     '  math("sq", [[\\sqrt[3]{<>}]], "yours", { priority = 1 }),',
     [("tex", I("‸"), ["sq"], None)])
case("C22b", "same trigger, no priority: which one wins is load order",
     draft("sq", [T("\\sqrt[3]{"), Tab(1), T("}")]),
     '  math("sq", [[\\sqrt[3]{<>}]], "yours"),',
     [("tex", I("‸"), ["sq"], None)])
case("C23", "check function",
     draft("aa", [T("\\alpha")], check='return line:find("=", 1, true) ~= nil'),
     '  math("aa", [[\\alpha]], "yours", { check = function(line, trigger, captures) return line:find("=", 1, true) ~= nil end }),',
     [("tex", I("x=‸"), ["aa"], None), ("tex", I("‸"), ["aa"], None)])
case("C24", 'quote and backslash in the trigger: \\q"',
     draft('\\q"', [T("\\quad")]),
     '  math("\\\\q\\"", [[\\quad]], "yours"),',
     [("tex", I("a‸"), ['\\q"'], None)],
     alts=['  math([[\\q"]], [[\\quad]], "yours"),'])
case("C25", "unicode in the output",
     draft("tto", [T("↦ ∘ α")]),
     '  math("tto", [[↦ ∘ α]], "yours"),',
     [("tex", I("‸"), ["tto"], None)])
case("C26", "unicode trigger",
     draft("→", [T("\\to")]),
     '  math("→", [[\\to]], "yours"),',
     [("tex", I("a‸"), ["→"], None)])
case("C27", "suffix",
     draft("tp", [T("^{\\top}")], kind="Suffix"),
     '  suffix("tp", [[^{\\top}]], "yours"),',
     [("tex", I("‸"), ["Atp"], None)])
case("C28", "maths on Tab (math_tab)",
     draft("ip", [T("\\langle "), Tab(1), T(", "), Tab(2), T(" \\rangle")], fires="Tab"),
     '  math_tab("ip", [[\\langle <>, <> \\rangle]], "yours"),',
     [("tex", I("‸"), ["ip<Tab>", "u<Tab>", "v<Tab>"], None), ("tex", I("‸"), ["ip"], None)])
case("C29", "display maths only, desc with a quote and a backslash, priority",
     draft("nrm", [T("\\|"), Tab(1), T("\\|")], math_where="Display", priority=2, desc='The "norm" \\| x \\|'),
     '  math("nrm", [[\\|<>\\|]], "yours", { where = "display", priority = 2, desc = "The \\"norm\\" \\\\| x \\\\|" }),',
     [("tex", D("‸"), ["nrm"], None), ("tex", I("‸"), ["nrm"], None)])
case("C30", "]] and ]=] both in the body → level 2",
     draft("lv2", [T("a]]b]=]c")]),
     '  math("lv2", [==[a]]b]=]c]==], "yours"),',
     [("tex", I("‸"), ["lv2"], None)])
case("C31", "a % inside a tab stop default is NOT doubled (helpers.parse reads defaults raw)",
     draft("pcd", [Tab(1, "50%"), T(" off")]),
     '  math("pcd", [[<:50%> off]], "yours"),',
     [("tex", I("‸"), ["pcd"], None)])
case("C32", "trailing newline in a text Tab snippet",
     draft("hrl", [T("\\hrule\n")], context="Text", fires="Tab", line="Start"),
     '  text("hrl", [[\\hrule\n]], "yours", { line = "start", tab = true }),',
     [("tex", "‸", ["hrl<Tab>"], None)],
     alts=['  text("hrl", [[\\hrule\n]], "yours", { tab = true, line = "start" }),'])
case("C33", "inline maths only (where = inline), text Tab auto in markdown too",
     draft("hlf", [T("\\tfrac12")], math_where="Inline"),
     '  math("hlf", [[\\tfrac12]], "yours", { where = "inline" }),',
     [("md", I("‸"), ["hlf"], None), ("md", "$$\n‸\n$$", ["hlf"], None)])
case("C34", "visual with a tab stop (no longer stays selected)",
     draft("E", [T("\\frac{"), SEL, T("}{"), Tab(1), T("}")], kind="Visual"),
     '  vis("E", [[\\frac{%V}{<>}]], "yours"),',
     [("tex", I("‸a+b"), ["E", "2<Tab>"], 3)])
case("C35", "pattern on Tab",
     draft("ve(%a)", [T("\\vec{"), Cap(1), T("}")], kind="Pattern", fires="Tab", eg="vex"),
     '  pat("ve(%a)", [[\\vec{%1}]], "yours", { tab = true, eg = "vex" }),',
     [("tex", I("‸"), ["vex<Tab>"], None)])
case("C36", "RawLua is written verbatim (trimmed, comma ensured)",
     draft("rw", [], kind="RawLua", raw_lua='math("rw", [[\\rho]], "yours")'),
     '  math("rw", [[\\rho]], "yours"),',
     [("tex", I("‸"), ["rw"], None)],
     alts=['math("rw", [[\\rho]], "yours"),'])

case("C37", "mirror carrying a default: only the first occurrence has it",
     draft("mrd", [Tab(1, "x"), T(" = "), Tab(1)]),
     '  math("mrd", [[<1:x> = <1>]], "yours"),',
     [("tex", I("‸"), ["mrd", "y<Tab>"], None)])
case("C38", "trailing newline in maths (display)",
     draft("nlz", [T("\\quad\n")]),
     '  math("nlz", [[\\quad\n]], "yours"),',
     [("tex", D("a ‸"), ["nlz"], None)])
case("C39", "Text + Tab, after text (line = after text)",
     draft("eqq", [T(" \\eqref{"), Tab(1), T("}")], context="Text", fires="Tab", line="AfterText"),
     '  text("eqq", [[ \\eqref{<>}]], "yours", { line = "after text", tab = true }),',
     [("tex", "‸", ["see eqq<Tab>"], None), ("tex", "‸", ["eqq<Tab>"], None)],
     alts=['  text("eqq", [[ \\eqref{<>}]], "yours", { tab = true, line = "after text" }),'])

POISON = {"C32", "C38"}  # LuaSnip errors mid-expansion and corrupts the session: measure them last

def ordered():
    return [c for c in CASES if c["id"] not in POISON] + [c for c in CASES if c["id"] in POISON]

def emit():
    out = [{"name": "list", "op": "list"}]
    for c in ordered():
        for k, (ft, text, steps, sel) in enumerate(c["typings"]):
            item = {"name": f'{c["id"]}#{k}', "entry": c["entry"], "ft": ft, "text": text, "steps": steps}
            if sel: item["select"] = sel
            out.append(item)
    json.dump(out, sys.stdout, ensure_ascii=False, indent=1)

if __name__ == "__main__":
    if sys.argv[1] == "emit":
        emit()

# ---------------------------------------------------------------- conflicts
# id, title, draft, entry, typings, expected finding codes (infer::check with the real known list)
CONFLICTS = []
def conflict(cid, title, d, entry, typings, findings, note=""):
    CONFLICTS.append(dict(id=cid, title=title, draft=d, entry=entry, typings=typings, findings=findings, note=note))

conflict("K01", "new auto 'in' shadows inn, int, infi (and suffix invs)",
         draft("in", [T("\\in")]), '  math("in", [[\\in]], "yours"),',
         [("tex", I("x ‸"), ["inn"], None), ("tex", I("‸"), ["int"], None)], ["shadows"])
conflict("K02", "new auto 'lim2' is shadowed by lim (symbols.lua): it can never be typed",
         draft("lim2", [T("\\lim_{n}")]), '  math("lim2", [[\\lim_{n}]], "yours"),',
         [("tex", I("‸"), ["lim2"], None)], ["shadowed-by"])
conflict("K03", "new auto 'srx' is shadowed mid-word by suffix sr",
         draft("srx", [T("\\sigma_r")]), '  math("srx", [[\\sigma_r]], "yours"),',
         [("tex", I("‸"), ["srx"], None)], ["shadowed-by"])
conflict("K04", "new auto 'pix' is shadowed by pi (greek.lua)",
         draft("pix", [T("\\pi x")]), '  math("pix", [[\\pi x]], "yours"),',
         [("tex", I("‸"), ["pix"], None)], ["shadowed-by"])
conflict("K05", "new text auto 'be' shadows the Tab shortcut beg (text.lua)",
         draft("be", [T("\\beta")], context="Text"), '  text("be", [[\\beta]], "yours"),',
         [("tex", "‸", ["beg<Tab>"], None)], ["shadows"])
conflict("K06", "new auto 'a1b' is shadowed by the auto subscript pattern at a1",
         draft("a1b", [T("\\aleph_1")]), '  math("a1b", [[\\aleph_1]], "yours"),',
         [("tex", I("‸"), ["a1b"], None)], ["shadowed-by"])
conflict("K07", "new auto 'su' shadows sum, sub, sup=",
         draft("su", [T("\\sigma_u")]), '  math("su", [[\\sigma_u]], "yours"),',
         [("tex", I("‸"), ["sum"], None)], ["shadows"])
conflict("K08", "new auto 'xx' duplicates symbols.lua xx: hers wins (load order) until priority 1",
         draft("xx", [T("\\chi")]), '  math("xx", [[\\chi]], "yours"),',
         [("tex", I("‸"), ["xx"], None)], ["duplicate"])
conflict("K09", "text 'sum' does not conflict with maths 'sum' (different context)",
         draft("sum", [T("summary")], context="Text", fires="Tab"), '  text("sum", [[summary]], "yours", { tab = true }),',
         [("tex", "‸", ["sum<Tab>"], None), ("tex", I("‸"), ["sum"], None)], [])
conflict("K10", "new vis 'B' duplicates visual.lua B",
         draft("B", [T("\\mathbf{"), SEL, T("}")], kind="Visual"), '  vis("B", [[\\mathbf{%V}]], "yours"),',
         [("tex", I("‸a+b"), ["B"], 3)], ["duplicate"])
conflict("K11", "new text auto 'the' fires inside ordinary English",
         draft("the", [T("\\theta")], context="Text"), '  text("the", [[\\theta]], "yours"),',
         [("tex", "‸", ["then"], None)], ["english-words"])
conflict("K12", "new math_tab 'par' duplicates calculus.lua par (Tab)",
         draft("par", [T("\\parallel")], fires="Tab"), '  math_tab("par", [[\\parallel]], "yours"),',
         [("tex", I("‸"), ["par<Tab>"], None)], ["duplicate"])
conflict("K13", "new auto 'ff' conflicts with nothing",
         draft("ff", [T("\\frac{"), Tab(1), T("}{"), Tab(2), T("}")]), '  math("ff", [[\\frac{<>}{<>}]], "yours"),',
         [("tex", I("‸"), ["ff"], None)], [])
conflict("K14", "new post 'hat' duplicates accents.lua post hat",
         draft("hat", [T("\\widehat{"), Cap(1), T("}")], kind="Post"), '  post("hat", [[\\widehat{%1}]], "yours"),',
         [("tex", I("‸"), ["xhat"], None)], ["duplicate"])

def emit_conflicts():
    out = []
    for c in CONFLICTS:
        for k, (ft, text, steps, sel) in enumerate(c["typings"]):
            item = {"name": f'{c["id"]}#{k}', "entry": c["entry"], "ft": ft, "text": text, "steps": steps}
            if sel: item["select"] = sel
            out.append(item)
            # and the same typing WITHOUT the new snippet (baseline)
            base = dict(item); base.pop("entry"); base["name"] = f'{c["id"]}#{k}-base'
            out.append(base)
    json.dump(out, sys.stdout, ensure_ascii=False, indent=1)

if __name__ == "__main__" and sys.argv[1] == "conflicts":
    emit_conflicts()
