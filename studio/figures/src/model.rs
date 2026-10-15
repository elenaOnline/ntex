//! The shared shapes of a figure. ADD-ONLY: add types, fields (with
//! #[serde(default)]) and variants, and report them; never rename or remove.
use serde::{Deserialize, Serialize};

/// A point in centimetres, y up (TikZ's frame).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Pt {
    pub x: f32,
    pub y: f32,
}

/// Where an object was placed.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Pos {
    /// Grid cell (column, row); rows go down. Fractions allowed.
    Cell(f32, f32),
    /// Absolute centimetres, y up.
    Abs(f32, f32),
}

/// Where a statement came from in the source (1-based line; for grid
/// blocks, the block's first line; `col` 1-based, 0 = unknown).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Head {
    #[default]
    To,
    /// ->>
    TwoHeads,
    /// >->
    Tail,
    /// hook->
    Hook,
    /// |->  (maps to)
    MapsTo,
    /// <->
    Both,
    /// = (no head, double line)
    Equal,
    /// =>
    Double,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Line {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LabelSide {
    /// TikZ-cd's default (left of the direction of travel)
    #[default]
    Auto,
    Swap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LabelPos {
    #[default]
    Middle,
    NearStart,
    NearEnd,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub name: String,
    /// LaTeX maths (no $), already converted from Unicode.
    pub label: String,
    #[serde(default)]
    pub pos: Option<Pos>,
    #[serde(default)]
    pub color: Option<String>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Arrow {
    /// Node, blob, element, ngon-vertex ("P.1") or cayley-vertex ("C.rs") name.
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub head: Head,
    #[serde(default)]
    pub line: Line,
    /// Degrees; + = bend left (TikZ `bend left`).
    #[serde(default)]
    pub bend: f32,
    #[serde(default)]
    pub is_loop: bool,
    #[serde(default)]
    pub side: LabelSide,
    #[serde(default)]
    pub label_pos: LabelPos,
    #[serde(default)]
    pub iso: bool,
    #[serde(default)]
    pub thick: bool,
    #[serde(default)]
    pub color: Option<String>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BlobShape {
    #[default]
    Round,
    Tall,
    Wide,
    Bean,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LabelPlace {
    #[default]
    Above,
    Below,
    Left,
    Right,
    Inside,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Element {
    pub name: String,
    pub label: String,
    #[serde(default)]
    pub pos: Option<Pos>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Blob {
    pub name: String,
    pub label: String,
    #[serde(default)]
    pub elements: Vec<Element>,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub shaded: bool,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub shape: BlobShape,
    /// (w, h) cm
    #[serde(default)]
    pub size: Option<(f32, f32)>,
    #[serde(default)]
    pub pos: Option<Pos>,
    #[serde(default)]
    pub label_place: LabelPlace,
    pub span: Span,
    /// (interaction pass) Rotation of the outline, degrees counter-clockwise (`angle=20`).
    #[serde(default)]
    pub angle: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NGon {
    pub name: String,
    pub n: u32,
    /// n labels (default "1".."n"), counter-clockwise from the top vertex.
    pub labels: Vec<String>,
    #[serde(default)]
    pub all_axes: bool,
    /// Axis through vertex k (1-based), or through edge k's midpoint when k
    /// is written k.5: stored as 2k (vertex) or 2k+1 (edge).
    #[serde(default)]
    pub axes: Vec<u32>,
    /// Rotation arrow by k steps (0 = none).
    #[serde(default)]
    pub rotation: u32,
    #[serde(default)]
    pub turn: u32,
    /// Reflect across this axis (same encoding as `axes`), applied after turn.
    #[serde(default)]
    pub flip: Option<u32>,
    #[serde(default)]
    pub radius: Option<f32>,
    #[serde(default)]
    pub pos: Option<Pos>,
    pub span: Span,
    /// (2026-10-12, interaction pass) Extra rotation of the drawing, degrees
    /// counter-clockwise (`angle=45`), on top of the default orientation.
    /// Unlike `turn`, labels rotate with the polygon.
    #[serde(default)]
    pub angle: f32,
    /// `labels=none` hides the vertex labels (the labels are kept, so turning
    /// them back on restores them). An empty string in `labels` hides just
    /// that vertex's label.
    #[serde(default = "yes")]
    pub show_labels: bool,
    /// The tinted fill; `plain` turns it off.
    #[serde(default = "yes")]
    pub shaded: bool,
    /// `color=…` (palette name); default blue.
    #[serde(default)]
    pub color: Option<String>,
    // (figure-qol D, direction and orientation) added together:
    /// The rotation arrow runs clockwise (`rotation cw`, `rotation=-2`): its
    /// default label is r^{-k}. `rotation` stays the (positive) step count.
    #[serde(default)]
    pub rotation_cw: bool,
    /// The edge (1-based; edge k joins vertex k and k+1, counter-clockwise)
    /// the rotation arrow sits outside (`rotation edge=3`). None = the
    /// upper-right edge, as before.
    #[serde(default)]
    pub rotation_edge: Option<u32>,
    /// `turn=-1` was written: `turn` still holds the same turn mod n
    /// (n−1), this only says how to write it back.
    #[serde(default)]
    pub turn_neg: bool,
    // ---- (figure-qol class C) label text of the parts that had none to set.
    /// The rotation arrow's label, when she changed it (`rotation "ρ"`);
    /// None = the default `r` / `r^{k}`.
    #[serde(default)]
    pub rotation_label: Option<String>,
    /// Labels at the ends of axes (`axis 1 "s"`): (encoded axis, LaTeX).
    #[serde(default)]
    pub axis_labels: Vec<(u32, String)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum GroupSpec {
    /// "D4", "Z6", "Q8", "S3", "Z2xZ3", …
    Named(String),
    /// generators and relators as written: (["a","b"], ["a^4","b^2","(ab)^2"])
    Presentation(Vec<String>, Vec<String>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CayleyLayout {
    #[default]
    Auto,
    Circle,
    Prism,
    Grid,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cayley {
    pub name: String,
    pub group: GroupSpec,
    /// Generators to draw edges for (as written in `<…>`).
    pub gens: Vec<String>,
    #[serde(default)]
    pub layout: CayleyLayout,
    #[serde(default = "yes")]
    pub labels: bool,
    #[serde(default)]
    pub legend: bool,
    #[serde(default)]
    pub radius: Option<f32>,
    #[serde(default)]
    pub pos: Option<Pos>,
    pub span: Span,
    /// (interaction pass) Rotation of the whole layout, degrees counter-clockwise.
    #[serde(default)]
    pub angle: f32,
    /// (figure-qol D) Per-generator look: colour, direction (g or g⁻¹),
    /// shown or hidden (`gens r:maroon,s^-1,t:hidden`). Only generators that
    /// differ from the default have an entry.
    #[serde(default)]
    pub gen_styles: Vec<GenStyle>,
    /// (figure-qol class C) Vertex names she gave: (element word, LaTeX).
    /// A vertex without an entry is named by its word.
    #[serde(default)]
    pub vertex_names: Vec<(String, String)>,
}

/// (figure-qol D) How one generator's edges of a Cayley graph are drawn.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct GenStyle {
    /// The generator as written in `<…>` (LaTeX, like `Cayley::gens`).
    pub generator: String,
    /// Palette name; None = the generator's own colour.
    #[serde(default)]
    pub color: Option<String>,
    /// Draw g⁻¹: the edges point from xg to x.
    #[serde(default)]
    pub inverse: bool,
    #[serde(default)]
    pub hidden: bool,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Text {
    /// LaTeX text-mode content.
    pub text: String,
    pub pos: Pos,
    pub span: Span,
}

/// One statement, in file order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Item {
    Node(Node),
    Arrow(Arrow),
    Blob(Blob),
    NGon(NGon),
    Cayley(Cayley),
    Text(Text),
    /// A `grid … end` block: its nodes and arrows, as the parser read them
    /// (also present as separate Node/Arrow items right after it, flagged by
    /// `from_grid` in Figure::grid_items) — the formatter writes the block
    /// back verbatim unless the canvas changed one of its objects.
    Grid { source: String, span: Span },
    /// A comment or blank line, kept so formatting preserves them.
    Comment { text: String, span: Span },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Globals {
    pub scale: f32,
    /// Cell size (cm).
    pub spacing: (f32, f32),
    /// "small" | "normal" | "large"
    pub size: String,
}

/// (FIG-POLISH) The default grid cell (cm): about what tikz-cd's default
/// `row sep` / `column sep` give for her notes' nodes (measured: a square of
/// single letters 1.41 × 1.14 cm, of `G`, `G/K`, `φ(G)` 1.88 × 1.24 cm).
pub const DEFAULT_SPACING: (f32, f32) = (1.9, 1.35);

impl Default for Globals {
    fn default() -> Self {
        Globals { scale: 1.0, spacing: DEFAULT_SPACING, size: "normal".into() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Figure {
    pub items: Vec<Item>,
    pub globals: Globals,
    /// Indices into `items` of the Node/Arrow items that came from grid
    /// blocks, with the index of their Grid item.
    #[serde(default)]
    pub grid_items: Vec<(usize, usize)>,
    /// (FIG-CORE-A) How the source was written, so `format` can write it back
    /// close to her text. Not part of the picture; layout ignores it.
    #[serde(default)]
    pub hints: Hints,
}

/// (FIG-CORE-A) Formatting memory that isn't part of the picture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Hints {
    /// Names of nodes with no statement of their own: created by their first
    /// mention in an arrow (`A -> B`). Their Node item sits right before
    /// that arrow, and the formatter writes their label / `at` inline there.
    #[serde(default)]
    pub implicit: Vec<String>,
    /// Blobs whose `{…}` also listed elements that live in a child blob
    /// (`blob G {e, g}` + `blob K in G {e}`): (blob, element names as
    /// written, in order). The element itself is only in the innermost blob.
    #[serde(default)]
    pub listed: Vec<(String, Vec<String>)>,
    /// (FIG-POLISH) Labels she typed in a spelling other than the LaTeX the
    /// model stores (`φ` for `\varphi`), so `format` writes them back as
    /// typed. Only entries whose raw form differs from `latex` are kept, in
    /// the order of the figure's label sites (see `spelling::sites`).
    #[serde(default)]
    pub spelling: Vec<Spelling>,
    /// (FIG-POLISH) Where each global setting was written: ("scale" |
    /// "spacing" | "size", number of items before it). `format` writes it
    /// back there; a global with no entry goes at the top.
    #[serde(default)]
    pub globals_at: Vec<(String, usize)>,
}

/// (FIG-POLISH) One label as she typed it. `key` names the label's site:
/// `node:A`, `arrow:A>B`, `blob:G`, `elem:g`, `ngon:P.2`, `cayley:C.1`,
/// `rel:C.1`, `group:C`, `text`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Spelling {
    pub key: String,
    /// The label as the model stores it (LaTeX).
    pub latex: String,
    /// The label as she wrote it.
    pub raw: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub span: Span,
    /// One sentence: "line 4: `->>` needs something to point at."
    pub message: String,
}

// ---------------------------------------------------------------------------
// the Scene: everything placed, in cm — what the canvas draws and TikZ emits

/// Which object a drawable belongs to, for hit-testing and selection.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ObjRef {
    Node(String),
    /// index into Figure::items
    Arrow(usize),
    Blob(String),
    Element(String),
    NGon(String),
    /// (cayley name, vertex word)
    CayleyVertex(String, String),
    /// (cayley name, edge index)
    CayleyEdge(String, usize),
    Text(usize),
    /// (ngon name, k): the k-th vertex position (1-based, counter-clockwise
    /// from the top), what an arrow names as "P.k". Added by FIG-CORE-B.
    NGonVertex(String, u32),
    /// A Cayley graph as a whole (its legend; hit-testing the graph).
    /// Added by FIG-CORE-B.
    Cayley(String),
    /// (figure-qol D) An n-gon's rotation arrow and its label, selectable as
    /// a part of its own.
    NGonRotation(String),
    /// (figure-qol class C) The label at the end of an n-gon's axis
    /// (ngon name, encoded axis). Only labels carry it; a click selects the n-gon.
    NGonAxis(String, u32),
    /// (figure-qol class C) The n-gon's rotation arrow's label: the target of
    /// editing it. The scene draws that label with `ObjRef::NGonRotation`, so
    /// a click on it selects the rotation arrow.
    NGonRotationLabel(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Seg {
    Line(Pt),
    /// cubic Bézier: c1, c2, end
    Cubic(Pt, Pt, Pt),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    /// palette name ("ink", "blue", …)
    pub color: String,
    /// pt
    pub width: f32,
    pub line: Line,
    #[serde(default)]
    pub double: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Draw {
    /// A LaTeX maths label (the canvas shows `unicode::preview(latex)`).
    Label { obj: ObjRef, at: Pt, latex: String, color: String, size: f32, math: bool },
    Path { obj: ObjRef, start: Pt, segs: Vec<Seg>, closed: bool, stroke: Option<Stroke>, fill: Option<String>, fill_opacity: f32, start_tip: Option<Head>, end_tip: Option<Head> },
    Dot { obj: ObjRef, at: Pt, r: f32, color: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Rect {
    pub min: Pt,
    pub max: Pt,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Scene {
    pub draws: Vec<Draw>,
    /// Every placed node / element / vertex, with its approximate box (for
    /// arrows, hit-testing, and the canvas's handles).
    pub anchors: Vec<(ObjRef, Pt, Rect)>,
    pub bounds: Rect,
    /// The effective grid cell in cm (spacing × scale), so the canvas and
    /// `recognize` can snap a point to `Pos::Cell`: x = col·cell.0,
    /// y = −row·cell.1. (0,0) = unknown → DEFAULT_SPACING. Added by FIG-CORE-B.
    #[serde(default)]
    pub cell: (f32, f32),
    /// (FIG-MODEL, interaction pass) Where each shape sits, for the
    /// window's resize / rotate handles: one entry per n-gon, blob and
    /// drawn Cayley graph, in item order.
    #[serde(default)]
    pub shapes: Vec<ShapeFrame>,
    /// (FIG-MODEL) The figure's `scale` (0 = unknown → 1). Lengths in the
    /// Scene are model lengths × scale: an n-gon's `radius=` is
    /// `ShapeFrame::radius / scale`, a blob's `size=` is `2·half / scale`.
    #[serde(default)]
    pub scale: f32,
}

/// (FIG-MODEL, interaction pass) A shape's frame in the Scene (cm, y up).
/// The handles go on the box `centre ± half`, turned `angle` degrees
/// counter-clockwise about `centre`; a resize dot on the circle of `radius`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShapeFrame {
    /// `ObjRef::NGon`, `ObjRef::Blob` or `ObjRef::Cayley`.
    pub obj: ObjRef,
    /// The rotation centre: the n-gon's centre, the blob's centre, the
    /// Cayley graph's centre (what `at` places).
    pub centre: Pt,
    /// N-gon: the circumradius (vertices lie on it; = `radius=` × scale).
    /// Blob: the half-diagonal of `half`. Cayley graph: the farthest
    /// vertex's distance from the centre (see `layout::cayley_radius_for`
    /// to turn a dragged distance into `radius=`).
    pub radius: f32,
    /// Half width and height in the shape's own (unturned) frame. N-gon:
    /// (radius, radius). Blob: its semi-axes, = `size=` / 2 × scale (the
    /// hand-drawn outline wanders up to 6 % past them). Cayley graph: the
    /// half extents of its vertices before turning.
    pub half: (f32, f32),
    /// How far the drawing is turned in the Scene, degrees counter-clockwise
    /// (= the item's `angle`, plus `inherited`).
    pub angle: f32,
    /// The part of `angle` that comes from an enclosing blob (a child blob
    /// turns with its parent). The item's own `angle` = `angle − inherited`.
    #[serde(default)]
    pub inherited: f32,
}

// ---------------------------------------------------------------------------
// canvas edits (applied to the Figure, then written back as source)

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Edit {
    AddNode { label: String, pos: Pos },
    MoveNode { name: String, pos: Pos },
    SetNodeLabel { name: String, label: String },
    AddArrow { from: String, to: String, label: Option<String> },
    /// replace an arrow item (index into items) with this arrow
    SetArrow { index: usize, arrow: Arrow },
    AddBlob { blob: Blob },
    MoveBlob { name: String, pos: Pos },
    AddElement { blob: String, label: String, pos: Option<Pos> },
    AddNGon { n: u32, pos: Pos, radius: f32 },
    Delete { obj: ObjRef },
    AddText { text: String, pos: Pos },
    // (2026-10-12, interaction pass)
    /// Replace any item wholesale (the properties panel builds the changed
    /// item and sends it here). Labels she didn't touch keep her spelling.
    SetItem { index: usize, item: Item },
    /// Move anything that has a position (node, blob, n-gon, Cayley graph,
    /// text, element) to `pos`.
    MoveObj { obj: ObjRef, pos: Pos },
    /// Copy an object next to itself (fresh names); returns the new name.
    Duplicate { obj: ObjRef },
}

/// What a rough canvas gesture was recognised as.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Recognized {
    /// A tap/click on empty space: a node goes here (snapped).
    Node(Pos),
    /// A stroke from one object to another.
    Arrow { from: String, to: String },
    /// A closed loop: a blob of this centre/size/shape (members: the existing
    /// elements and blobs it encloses).
    Blob { center: Pt, size: (f32, f32), shape: BlobShape, encloses: Vec<String> },
    /// A closed polyline with k corners: a regular k-gon.
    NGon { n: u32, center: Pt, radius: f32 },
    /// An open stroke that is none of the above: nothing (the canvas says so).
    Nothing,
}
