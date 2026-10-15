"""Build verification/expected.json from cases.py + the measured runs (measured.json,
measured_conflicts.json, known.json, files.json).  python3 -I gen_expected.py OUTDIR"""
import json, sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cases import CONFLICTS, ordered, T, Tab, EXIT, SEL, Cap, draft
CASES = ordered()

HERE = os.path.dirname(os.path.abspath(__file__))
def load(n): return json.load(open(os.path.join(HERE, "measured", n)))
measured = {c["name"]: c for c in load("measured.json")[1:]}
measured_k = {c["name"]: c for c in load("measured_conflicts.json")}
known_raw = load("known.json")
files = load("files.json")

# What ntex SHOULD do where today's measurement shows the escape missing or the
# trailing-newline bug (derivation in acceptance.md).
OVERRIDE = {
    "C32#0": ["\\hrule\n‸"],
    "C38#0": ["\\[\na \\quad\n‸\n\\]"],
}
SEMANTIC_ENTRY = {"C03", "C32", "C38"}  # exact text not required; must load + expand right

def steps_of(name, src):
    m = src[name]
    out = []
    for k, s in enumerate(m["steps"]):
        st = {"keys": s["keys"], "buffer": s["buffer"], "mode": s["mode"], "sel": s.get("sel"),
              "expanded": [e["file"] for e in s["expanded"]]}
        if name in OVERRIDE:
            st["buffer"] = OVERRIDE[name][k]; st["mode"] = "i"; st["sel"] = None
            st["expanded"] = ["zz_adv.lua"] if k == 0 else []
            st["measured_today"] = s["buffer"]
        out.append(st)
    return out

snips = []
for c in CASES:
    tries = []
    for k, (ft, text, keys, sel) in enumerate(c["typings"]):
        tries.append({"ft": ft, "text": text, "select": sel, "steps": steps_of(f'{c["id"]}#{k}', measured)})
    snips.append({"id": c["id"], "title": c["title"], "draft": c["draft"], "entry": c["entry"],
                  "alts": c["alts"], "semantic": c["id"] in SEMANTIC_ENTRY, "note": c["note"], "tries": tries})

conflicts = []
for c in CONFLICTS:
    tries = []
    for k, (ft, text, keys, sel) in enumerate(c["typings"]):
        tries.append({"ft": ft, "text": text, "select": sel,
                      "with": steps_of(f'{c["id"]}#{k}', measured_k),
                      "without": steps_of(f'{c["id"]}#{k}-base', measured_k)})
    conflicts.append({"id": c["id"], "title": c["title"], "draft": c["draft"], "entry": c["entry"],
                      "findings": c["findings"], "tries": tries})

known = []
for m in known_raw:
    known.append({"trigger": m["trigger"], "pattern": m.get("pattern"), "kind": m["kind"], "where": m["where"],
                  "how": m["how"], "body": m.get("body"), "file": m["file"], "line": m.get("line"),
                  "origin": "yours", "group": None, "line_rule": m.get("line_rule"), "ft": m.get("ft"),
                  "priority": m.get("priority", 0), "desc": None})

parse_body = [
    ("\\frac{<>}{<>}", [T("\\frac{"), Tab(1), T("}{"), Tab(2), T("}")]),
    ("\\frac{$1}{$2}$0", [T("\\frac{"), Tab(1), T("}{"), Tab(2), T("}"), EXIT]),
    ("${1:x} + $1", [Tab(1, "x"), T(" + "), Tab(1)]),
    ("\\sqrt{${VISUAL}}", [T("\\sqrt{"), SEL, T("}")]),
    ("\\sqrt{$TM_SELECTED_TEXT}", [T("\\sqrt{"), SEL, T("}")]),
    ("\\int_{<:0>}^{<:1>} <> \\, d<:x> ", [T("\\int_{"), Tab(1, "0"), T("}^{"), Tab(2, "1"), T("} "), Tab(3), T(" \\, d"), Tab(4, "x"), T(" ")]),
    ("\\begin{<1>}\n\t<2>\n\\end{<1>}", [T("\\begin{"), Tab(1), T("}\n\t"), Tab(2), T("\n\\end{"), Tab(1), T("}")]),
    ("<2><>", [Tab(2), Tab(1)]),
    ("\\hat{%1}", [T("\\hat{"), Cap(1), T("}")]),
    ("%1_{%2}", [Cap(1), T("_{"), Cap(2), T("}")]),
    ("\\sqrt{ %V }", [T("\\sqrt{ "), SEL, T(" }")]),
    ("50\\%%", [T("50\\%")]),
    ("a%<%>b", [T("a<>b")]),
    ("a<b>c", [T("a<b>c")]),
    ("\\alpha + \\beta", [T("\\alpha + \\beta")]),
    ("<0>x", [EXIT, T("x")]),
]
renumber = [
    ([Tab(2), T("+"), Tab(1, "y"), T("+"), Tab(2)], [Tab(1), T("+"), Tab(2, "y"), T("+"), Tab(1)]),
    ([Tab(3), Tab(7)], [Tab(1), Tab(2)]),
    ([T("a"), EXIT, Tab(5, "q")], [T("a"), EXIT, Tab(1, "q")]),
]
lua_strings = ["\\frac{<>}{<>}", "a]]b", "\\left[ x \\right]", "]", "a]=]b]]", "\nfoo", "\n", "x\n", "tab\there", "", "q\"\\", "]==]]]"]

def D(**kw):
    out = kw.pop("output", [T("x")])
    return draft(kw.pop("trigger", "zq"), out, **kw)

guess = [
    {"why": "%V in the output → Visual", "draft": D(trigger="D", output=[T("\\bm{"), SEL, T("}")]), "place": None, "expect": {"kind": "Visual"}},
    {"why": "%1 + a letters trigger → Post", "draft": D(trigger="bbm", output=[T("\\mathbb{"), Cap(1), T("}")]), "place": None, "expect": {"kind": "Post"}},
    {"why": "\\section{ is text", "draft": D(trigger="sec", output=[T("\\section{"), Tab(1), T("}")]), "place": None, "expect": {"context": "Text"}},
    {"why": "\\frac is maths", "draft": D(trigger="ff", output=[T("\\frac{"), Tab(1), T("}{"), Tab(2), T("}")]), "place": None, "expect": {"context": "Math"}},
    {"why": "text trigger that starts English words → Tab", "draft": D(trigger="the", output=[T("\\section{"), Tab(1), T("}")]), "place": "Text", "expect": {"context": "Text", "fires": "Tab"}},
    {"why": "pinned fields are never re-guessed", "draft": dict(D(trigger="D", output=[T("\\bm{"), SEL, T("}")]), kind="Plain", pinned=["kind"]), "place": None, "expect": {"kind": "Plain"}},
    {"why": "pinned context survives a text-looking output", "draft": dict(D(trigger="sec", output=[T("\\section{"), Tab(1), T("}")]), context="Math", pinned=["context"]), "place": None, "expect": {"context": "Math"}},
]
checks = [
    {"why": "empty trigger", "draft": D(trigger=""), "codes": ["empty-trigger"], "errors": True},
    {"why": "empty output", "draft": D(output=[]), "codes": [], "errors": True},
    {"why": "visual without %V", "draft": D(trigger="D", kind="Visual", output=[T("\\bm{x}")]), "codes": ["no-selection"], "errors": None},
    {"why": "post without %1", "draft": D(trigger="bbm", kind="Post", output=[T("\\mathbb{R}")]), "codes": ["no-capture"], "errors": None},
    {"why": "text + pattern", "draft": D(trigger="(%a)q", kind="Pattern", context="Text", output=[Cap(1)], eg="xq"), "codes": [], "errors": True},
    {"why": "a clean draft has no errors", "draft": D(trigger="ff", output=[T("\\frac{"), Tab(1), T("}{"), Tab(2), T("}")]), "codes": [], "errors": False},
    {"why": "literal <> is noted (escaped)", "draft": D(trigger="ltg", output=[T("a<>b")]), "codes": ["literal-angle"], "errors": False},
    {"why": "tab-stop default holding > cannot be written (helpers.parse stops at >)", "draft": D(trigger="gtd", output=[Tab(1, "a>b")]), "codes": [], "errors": True,
     "alt": "or codegen escapes it so ntex shows the default a>b"},
    {"why": "trailing newline breaks LuaSnip (helpers trailing_ws) unless an exit follows", "draft": D(trigger="nlz", output=[T("\\quad\n")]), "codes": [], "errors": None,
     "alt": "acceptable: an Error/Warning finding, or codegen appends <0> (then e2e must give \\quad\\n‸)"},
]
try_plan = [
    {"draft": D(trigger="ff", output=[T("\\frac{"), Tab(1), T("}{"), Tab(2), T("}")]), "expect": {"ft": "tex", "place": "inline", "keys": "ff", "tab": False, "nfills": 2}},
    {"draft": D(trigger="ip", fires="Tab", output=[T("<"), Tab(1), T(">")]), "expect": {"place": "inline", "keys": "ip", "tab": True, "nfills": 1}},
    {"draft": D(trigger="bbm", kind="Post", output=[T("\\mathbb{"), Cap(1), T("}")]), "expect": {"keys_endswith": "bbm", "keys_len": 4, "tab": False}},
    {"draft": D(trigger="tp", kind="Suffix", output=[T("^{\\top}")]), "expect": {"keys_endswith": "tp", "keys_len": 3}},
    {"draft": D(trigger="(%a)vv(%d)", kind="Pattern", eg="xvv2", output=[Cap(1)]), "expect": {"keys": "xvv2"}},
    {"draft": D(trigger="D", kind="Visual", output=[T("\\bm{"), SEL, T("}")]), "expect": {"place": "visual", "keys": "D"}},
    {"draft": D(trigger="thm", context="Text", fires="Tab", line="Start", output=[T("x")]), "expect": {"place": "text-start", "tab": True}},
    {"draft": D(trigger="todo", context="Text", line="ListItem", output=[T("x")]), "expect": {"place": "list-item"}},
    {"draft": D(trigger="nrm", math_where="Display", output=[T("x")]), "expect": {"place": "display"}},
    {"draft": D(trigger="mdq", ft="Markdown", output=[T("x")]), "expect": {"ft": "markdown"}},
    {"draft": D(trigger="sect", context="Text", fires="Tab", line="Start", ft="Tex", output=[T("x")]), "expect": {"ft": "tex", "place": "text-start"}},
]

blocks = [
    {"id": "BL1", "why": "Fun fact like remark (unnumbered, upright, Green)",
     "block": {"title": "Fun fact", "env": "funfact", "base": "Remark", "numbering": "None", "body": "Upright",
               "colour": {"Palette": "Green"}, "cref": ["fun fact", "fun facts"], "trigger": "fun", "with_title": True},
     "stanza_has": ["% ── block: funfact ──", "% ── end: funfact ──", "\\newtheorem*{funfact}{Fun fact}", "\\theoremstyle{ntexupright}", "ntexbox=Green"],
     "stanza_lacks": ["\\definecolor", "\\crefname"],
     "doc": "\\begin{funfact}[Why] F. \\end{funfact}", "pdf_has": ["Fun fact (Why)."],
     "snippet_entry": '  text("fun", [[\\begin{funfact}[<>]\n\t<>\n\\end{funfact}]], "yours", { line = "start", tab = true, ft = "tex" }),',
     "snippet_type": {"keys": ["fun<Tab>", "Why<Tab>", "body<Tab>"],
                      "buffers": ["\\begin{funfact}[‸]\n\t\n\\end{funfact}", "\\begin{funfact}[Why]\n\t‸\n\\end{funfact}", "\\begin{funfact}[Why]\n\tbody\n\\end{funfact}‸"]}},
    {"id": "BL2", "why": "numbered with theorems, italic, Blue; \\cref must read like 'Theorem 1.1'",
     "block": {"title": "Fun fact", "env": "funfact", "base": "Theorem", "numbering": "WithTheorems", "body": "Italic",
               "colour": {"Palette": "Blue"}, "cref": ["fun fact", "fun facts"], "trigger": "", "with_title": False},
     "stanza_has": ["\\newtheorem{funfact}[theorem]{Fun fact}", "\\theoremstyle{ntexplain}", "ntexbox=Blue"],
     "stanza_lacks": ["\\newtheorem*"],
     "doc": "\\begin{funfact}\\label{ff:x} F. \\end{funfact} See \\cref{ff:x} and \\cref{thm:a}.",
     "pdf_has": ["See Fun fact 1.2 and Theorem 1.1."]},
    {"id": "BL3", "why": "own counter per section, new hue 145°",
     "block": {"title": "Aside", "env": "aside", "base": "Definition", "numbering": "Own", "body": "Upright",
               "colour": {"Hue": 145.0}, "cref": ["aside", "asides"], "trigger": "asd", "with_title": False},
     "stanza_has": ["\\definecolor{ntexAsideRule}", "\\definecolor{ntexAsideBack}", "\\definecolor{ntexAsideHead}", "ntexbox=Aside", "\\newtheorem{aside}{Aside}[section]"],
     "stanza_lacks": [],
     "doc": "\\begin{aside}\\label{as:y} A. \\end{aside} See \\cref{as:y}.", "pdf_has": ["Aside 1.1.", "See Aside 1.1."]},
    {"id": "BL4", "why": "the neutral Proof palette: ntexProofHead does not exist in ntex-notes.sty",
     "block": {"title": "Note", "env": "note", "base": "Remark", "numbering": "None", "body": "Upright",
               "colour": {"Palette": "Proof"}, "cref": ["note", "notes"], "trigger": "", "with_title": False},
     "stanza_has": [], "stanza_lacks": [], "doc": "\\begin{note} N. \\end{note}", "pdf_has": ["Note. N."]},
    {"id": "BL5", "why": "a title with TeX specials must be escaped",
     "block": {"title": "Q&A 50%", "env": "qa", "base": "Example", "numbering": "None", "body": "Upright",
               "colour": {"Palette": "Maroon"}, "cref": ["q&a", "q&as"], "trigger": "", "with_title": False},
     "stanza_has": [], "stanza_lacks": [], "doc": "\\begin{qa} Q. \\end{qa}", "pdf_has": ["Q&A 50%. Q."]},
]
block_checks = [
    {"why": "env taken by the theme", "block": dict(blocks[0]["block"], env="remark", title="Remark"), "has_error": True},
    {"why": "env not letters", "block": dict(blocks[0]["block"], env="fun-fact"), "has_error": True},
    {"why": "empty title", "block": dict(blocks[0]["block"], title=""), "has_error": True},
    {"why": "standard LaTeX env", "block": dict(blocks[0]["block"], env="itemize"), "has_error": True},
    {"why": "a fine block", "block": blocks[0]["block"], "has_error": False},
]
env_from_title = [["Fun fact", "funfact"], ["Côté", "cote"], ["Q&A 50%", "qa"], ["Claim 2", "claim"]]

ink = "1E1E28"
palette = {"Blue": ["1E47B8", "E0E7FB", "153079"], "Violet": ["661EB8", "ECE0FB", "441579"], "Teal": ["1C829C", "E2F4F8", "0F596B"],
           "Maroon": ["A92D4A", "F8EAED", "79152D"], "Green": ["238B45", "E5F5EA", "145D2C"]}
colour = {"palette": palette, "ink": ink,
          "contrast": [["1E1E28", "E0E7FB", 13.36], ["153079", "E0E7FB", 9.82], ["0F596B", "E2F4F8", 6.96], ["FFFFFF", "000000", 21.0], ["8A6A00", "F8F1DC", 4.49]],
          "hue_sweep": [0, 30, 60, 90, 145, 200, 260, 300, 330], "min_body": 7.0, "min_head": 4.5}

apply_cases = {
    "inbox": "\n".join([
        "-- Snippets made in Ntex Studio, waiting to be sorted. Safe to edit by hand.",
        'local h = require("snips.helpers")',
        "local math, math_tab, text, suffix, post, pat, vis = h.math, h.math_tab, h.text, h.suffix, h.post, h.pat, h.vis",
        "",
        'return h.group("Inbox", {',
        "  -- studio: id=bbm-1 made=2026-10-07T14:30",
        '  post("bbm", [[\\mathbb{%1}]], "yours"),',
        "  -- studio: id=thm-1 made=2026-10-07T14:31",
        '  text("thm", [[\\begin{theorem}',
        "\t<>",
        '\\end{theorem}]], "yours", { tab = true, line = "start" }),',
        "  -- studio: id=pr-1 made=2026-10-07T14:32",
        '  math("prb", [[\\Pr\\left( <> \\right)]], "yours"),',
        "  -- studio: id=lrs-1 made=2026-10-07T14:33",
        '  math("lrs", [=[\\left[ <> \\right]]=], "yours"),',
        "  -- studio: id=tp-1 made=2026-10-07T14:34",
        '  suffix("tp", [[^{\\top}]], "yours"),',
        "  -- studio: id=gqq-1 made=2026-10-07T14:35",
        '  math("gqq", [[\\geqq]], "yours"),',
        "  -- studio: id=stay-1 made=2026-10-07T14:36",
        '  math("stq", [[\\star]], "yours"),',
        "})",
        ""]),
    "plan": {"moves": [
        {"id": "bbm-1", "file": "operations.lua", "after_trigger": "ee", "new_title": None, "reason": "a post accent next to the powers"},
        {"id": "thm-1", "file": "text.lua", "after_trigger": "beg", "new_title": None, "reason": "with the other Tab environments"},
        {"id": "pr-1", "file": "probability.lua", "after_trigger": None, "new_title": "Probability", "reason": "new topic"},
        {"id": "lrs-1", "file": "brackets.lua", "after_trigger": "no-such-trigger", "new_title": None, "reason": "brackets"},
        {"id": "tp-1", "file": "chemistry.lua", "after_trigger": "pu", "new_title": None, "reason": "a file whose locals have only math"},
        {"id": "gqq-1", "file": "symbols.lua", "after_trigger": "geq", "new_title": None, "reason": "her two-per-line edit"},
        {"id": "ghost-9", "file": "sets.lua", "after_trigger": None, "new_title": None, "reason": "not in the inbox"},
    ], "notes": ""},
    "expect": {
        "moved": {"bbm-1": "operations.lua", "thm-1": "text.lua", "pr-1": "probability.lua", "lrs-1": "brackets.lua", "tp-1": "chemistry.lua", "gqq-1": "symbols.lua"},
        "inbox_keeps": ["stq"],
        "order": {"operations.lua": ["ee", "bbm", "invs"], "text.lua": ["beg", "thm", "enum"], "chemistry.lua": ["pu", "tp", "cee"],
                  "symbols.lua": ["geq", "gqq", "leq"], "brackets.lua": ["lrs"]},
        "new_file_title": {"probability.lua": "Probability"},
        "type": [
            {"ft": "tex", "text": "$‸$", "keys": "Rbbm", "buffer": "$\\mathbb{R}‸$"},
            {"ft": "tex", "text": "‸", "keys": "thm<Tab>", "buffer": "\\begin{theorem}\n\t‸\n\\end{theorem}"},
            {"ft": "tex", "text": "$‸$", "keys": "prb", "buffer": "$\\Pr\\left( ‸ \\right)$"},
            {"ft": "tex", "text": "$‸$", "keys": "lrs", "buffer": "$\\left[ ‸ \\right]$"},
            {"ft": "tex", "text": "$‸$", "keys": "Atp", "buffer": "$A^{\\top}‸$"},
            {"ft": "tex", "text": "$a ‸$", "keys": "gqq", "buffer": "$a \\geqq‸$"},
            {"ft": "tex", "text": "$a ‸$", "keys": "stq", "buffer": "$a \\star‸$"},
        ],
    },
}

out = {
    "about": "Ntex Studio acceptance data, ADVERSARY. Buffers use ‸ for the cursor (insert mode). Measured in a real headless ntex "
             "(verification/try.lua) on 2026-10-07 unless 'measured_today' shows the value differs (then the value here is derived "
             "from the spec; see acceptance.md).",
    "snippets": snips, "conflicts": conflicts, "known": known, "files": files,
    "parse_body": [{"input": a, "pieces": b} for a, b in parse_body],
    "renumber": [{"input": a, "output": b} for a, b in renumber],
    "lua_strings": lua_strings,
    "guess": guess, "check": checks, "try_plan": try_plan,
    "blocks": blocks, "block_checks": block_checks, "env_from_title": env_from_title, "colour": colour,
    "apply": apply_cases,
}
json.dump(out, open(os.path.join(sys.argv[1], "expected.json"), "w"), ensure_ascii=False, indent=1)
print("ok", len(snips), "snippet cases,", len(conflicts), "conflicts")
