//! Every Ghostty option that changes how the terminal looks, grouped, with the
//! most visual control its type allows. The keys, defaults and docs come from
//! Ghostty itself (`ghostty +show-config --default --docs`): `LOOK` only adds
//! a group, a label and a kind. `NON_LOOK` names every other key, so a newer
//! Ghostty's new options can't slip by: the unit test fails on a key that is
//! in neither, and at runtime such a key lands in the "Other" group.

/// How an option is edited.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// `#rrggbb`. `optional`: may be unset (Ghostty picks). `special`: words
    /// it also takes (`cell-foreground`, `bright`, …).
    Color { optional: bool, special: &'static [&'static str] },
    /// A number with a range and a step (a slider / stepper).
    Number { min: f32, max: f32, step: f32, unit: &'static str },
    /// `adjust-*`: unset, whole pixels (`2`, `-1`) or a percent (`10%`).
    Metric,
    Bool,
    /// One of these words (the first is not necessarily the default).
    Choice(&'static [&'static str]),
    /// A font family (repeatable: fallbacks).
    Font,
    /// `window-padding-*`: `n` or `left,right` / `top,bottom`.
    Padding,
    /// Anything else, as Ghostty writes it.
    Text,
}

#[derive(Clone, Copy, Debug)]
pub struct Opt {
    pub key: &'static str,
    pub group: &'static str,
    pub label: &'static str,
    pub kind: Kind,
}

pub const GROUPS: &[&str] = &["Colors", "Palette", "Font", "Cell and glyphs", "Cursor", "Window", "Background image", "Splits", "Shader", "App icon", "Other"];

const CELL: &[&str] = &["cell-foreground", "cell-background"];
const NONE: &[&str] = &[];

const fn c(key: &'static str, group: &'static str, label: &'static str, optional: bool, special: &'static [&'static str]) -> Opt {
    Opt { key, group, label, kind: Kind::Color { optional, special } }
}
const fn n(key: &'static str, group: &'static str, label: &'static str, min: f32, max: f32, step: f32, unit: &'static str) -> Opt {
    Opt { key, group, label, kind: Kind::Number { min, max, step, unit } }
}
const fn o(key: &'static str, group: &'static str, label: &'static str, kind: Kind) -> Opt {
    Opt { key, group, label, kind }
}

/// The look options, in the order the file writes them.
pub const LOOK: &[Opt] = &[
    // the colors
    c("background", "Colors", "Background", false, NONE),
    c("foreground", "Colors", "Foreground", false, NONE),
    c("cursor-color", "Colors", "Cursor", true, CELL),
    c("cursor-text", "Colors", "Cursor text", true, CELL),
    c("selection-background", "Colors", "Selection", true, CELL),
    c("selection-foreground", "Colors", "Selection text", true, CELL),
    c("bold-color", "Colors", "Bold text", true, &["bright"]),
    n("minimum-contrast", "Colors", "Minimum contrast", 1.0, 21.0, 0.1, ":1"),
    n("faint-opacity", "Colors", "Faint text", 0.0, 1.0, 0.05, ""),
    c("search-background", "Colors", "Search match", false, CELL),
    c("search-foreground", "Colors", "Search match text", false, CELL),
    c("search-selected-background", "Colors", "Current match", false, CELL),
    c("search-selected-foreground", "Colors", "Current match text", false, CELL),
    o("palette-generate", "Colors", "Generate 16–255", Kind::Bool),
    o("palette-harmonious", "Colors", "Harmonious 16–255", Kind::Bool),
    o("alpha-blending", "Colors", "Alpha blending", Kind::Choice(&["native", "linear", "linear-corrected"])),
    o("window-colorspace", "Colors", "Color space", Kind::Choice(&["srgb", "display-p3"])),
    o("theme", "Colors", "Theme", Kind::Text),
    // the font
    o("font-family", "Font", "Family", Kind::Font),
    o("font-family-bold", "Font", "Bold family", Kind::Font),
    o("font-family-italic", "Font", "Italic family", Kind::Font),
    o("font-family-bold-italic", "Font", "Bold italic family", Kind::Font),
    n("font-size", "Font", "Size", 6.0, 36.0, 0.5, " pt"),
    o("font-style", "Font", "Regular style", Kind::Text),
    o("font-style-bold", "Font", "Bold style", Kind::Text),
    o("font-style-italic", "Font", "Italic style", Kind::Text),
    o("font-style-bold-italic", "Font", "Bold italic style", Kind::Text),
    o("font-synthetic-style", "Font", "Synthetic styles", Kind::Text),
    o("font-feature", "Font", "Features", Kind::Text),
    o("font-variation", "Font", "Variation axes", Kind::Text),
    o("font-variation-bold", "Font", "Bold variation", Kind::Text),
    o("font-variation-italic", "Font", "Italic variation", Kind::Text),
    o("font-variation-bold-italic", "Font", "Bold italic variation", Kind::Text),
    o("font-thicken", "Font", "Thicken", Kind::Bool),
    n("font-thicken-strength", "Font", "Thicken strength", 0.0, 255.0, 5.0, ""),
    o("font-codepoint-map", "Font", "Codepoint map", Kind::Text),
    o("font-shaping-break", "Font", "Shaping breaks", Kind::Text),
    o("window-title-font-family", "Font", "Title font", Kind::Font),
    // cell metrics
    o("adjust-cell-width", "Cell and glyphs", "Cell width", Kind::Metric),
    o("adjust-cell-height", "Cell and glyphs", "Cell height", Kind::Metric),
    o("adjust-font-baseline", "Cell and glyphs", "Baseline", Kind::Metric),
    o("adjust-underline-position", "Cell and glyphs", "Underline position", Kind::Metric),
    o("adjust-underline-thickness", "Cell and glyphs", "Underline thickness", Kind::Metric),
    o("adjust-strikethrough-position", "Cell and glyphs", "Strikethrough position", Kind::Metric),
    o("adjust-strikethrough-thickness", "Cell and glyphs", "Strikethrough thickness", Kind::Metric),
    o("adjust-overline-position", "Cell and glyphs", "Overline position", Kind::Metric),
    o("adjust-overline-thickness", "Cell and glyphs", "Overline thickness", Kind::Metric),
    o("adjust-cursor-thickness", "Cell and glyphs", "Cursor thickness", Kind::Metric),
    o("adjust-cursor-height", "Cell and glyphs", "Cursor height", Kind::Metric),
    o("adjust-box-thickness", "Cell and glyphs", "Box line thickness", Kind::Metric),
    o("adjust-icon-height", "Cell and glyphs", "Icon height", Kind::Metric),
    // the cursor
    o("cursor-style", "Cursor", "Shape", Kind::Choice(&["block", "bar", "underline", "block_hollow"])),
    o("cursor-style-blink", "Cursor", "Blink", Kind::Choice(&["", "true", "false"])),
    n("cursor-opacity", "Cursor", "Opacity", 0.0, 1.0, 0.05, ""),
    // the window
    o("window-padding-x", "Window", "Padding left, right", Kind::Padding),
    o("window-padding-y", "Window", "Padding top, bottom", Kind::Padding),
    o("window-padding-balance", "Window", "Balance padding", Kind::Bool),
    o("window-padding-color", "Window", "Padding color", Kind::Choice(&["background", "extend", "extend-always"])),
    n("background-opacity", "Window", "Opacity", 0.0, 1.0, 0.01, ""),
    o("background-opacity-cells", "Window", "Opacity of colored cells", Kind::Bool),
    o("background-blur", "Window", "Blur", Kind::Choice(&["false", "true", "10", "40", "macos-glass-regular", "macos-glass-clear"])),
    o("window-theme", "Window", "Window theme", Kind::Choice(&["auto", "system", "light", "dark", "ghostty"])),
    o("macos-titlebar-style", "Window", "Title bar", Kind::Choice(&["transparent", "native", "tabs", "hidden"])),
    o("macos-titlebar-proxy-icon", "Window", "Title bar file icon", Kind::Choice(&["visible", "hidden"])),
    o("macos-window-buttons", "Window", "Window buttons", Kind::Choice(&["visible", "hidden"])),
    o("macos-window-shadow", "Window", "Shadow", Kind::Bool),
    o("window-decoration", "Window", "Decorations", Kind::Choice(&["auto", "client", "server", "none"])),
    c("window-titlebar-background", "Window", "Title bar color (GTK)", true, NONE),
    c("window-titlebar-foreground", "Window", "Title bar text (GTK)", true, NONE),
    o("window-show-tab-bar", "Window", "Tab bar", Kind::Choice(&["auto", "always", "never"])),
    o("scrollbar", "Window", "Scrollbar", Kind::Choice(&["system", "never"])),
    o("progress-style", "Window", "Progress bar", Kind::Bool),
    o("resize-overlay", "Window", "Size overlay", Kind::Choice(&["after-first", "always", "never"])),
    o("resize-overlay-position", "Window", "Size overlay position", Kind::Choice(&["center", "top-left", "top-center", "top-right", "bottom-left", "bottom-center", "bottom-right"])),
    o("resize-overlay-duration", "Window", "Size overlay time", Kind::Text),
    // the background image
    o("background-image", "Background image", "Image", Kind::Text),
    n("background-image-opacity", "Background image", "Opacity", 0.0, 2.0, 0.05, ""),
    o("background-image-position", "Background image", "Position", Kind::Choice(&["center", "top-left", "top-center", "top-right", "center-left", "center-right", "bottom-left", "bottom-center", "bottom-right"])),
    o("background-image-fit", "Background image", "Fit", Kind::Choice(&["contain", "cover", "stretch", "none"])),
    o("background-image-repeat", "Background image", "Repeat", Kind::Bool),
    // splits
    n("unfocused-split-opacity", "Splits", "Unfocused opacity", 0.15, 1.0, 0.05, ""),
    c("unfocused-split-fill", "Splits", "Unfocused fill", true, NONE),
    c("split-divider-color", "Splits", "Divider", true, NONE),
    // shaders
    o("custom-shader", "Shader", "Shader file", Kind::Text),
    o("custom-shader-animation", "Shader", "Animate", Kind::Choice(&["true", "false", "always"])),
    // the app icon
    o("macos-icon", "App icon", "Icon", Kind::Choice(&["official", "blueprint", "chalkboard", "microchip", "glass", "holographic", "paper", "retro", "xray", "custom", "custom-style"])),
    o("macos-icon-frame", "App icon", "Frame", Kind::Choice(&["aluminum", "beige", "plastic", "chrome"])),
    c("macos-icon-ghost-color", "App icon", "Ghost color", true, NONE),
    o("macos-icon-screen-color", "App icon", "Screen colors", Kind::Text),
    o("macos-custom-icon", "App icon", "Custom icon file", Kind::Text),
];

/// Repeatable keys: each line adds a value; an empty value clears the list.
pub const REPEATABLE: &[&str] = &[
    "font-family",
    "font-family-bold",
    "font-family-italic",
    "font-family-bold-italic",
    "font-feature",
    "font-variation",
    "font-variation-bold",
    "font-variation-italic",
    "font-variation-bold-italic",
    "font-codepoint-map",
    "custom-shader",
];

/// Every other key Ghostty 1.3 has: behavior, not looks.
pub const NON_LOOK: &[&str] = &[
    "abnormal-command-exit-runtime", "app-notifications", "async-backend", "auto-update", "auto-update-channel", "bell-audio-path",
    "bell-audio-volume", "bell-features", "class", "click-repeat-interval", "clipboard-codepoint-map", "clipboard-paste-bracketed-safe",
    "clipboard-paste-protection", "clipboard-read", "clipboard-trim-trailing-spaces", "clipboard-write", "command", "command-palette-entry",
    "config-default-files", "config-file", "confirm-close-surface", "copy-on-select", "cursor-click-to-move", "desktop-notifications",
    "enquiry-response", "env", "focus-follows-mouse", "freetype-load-flags", "fullscreen", "grapheme-width-method", "gtk-custom-css",
    "gtk-opengl-debug", "gtk-quick-terminal-layer", "gtk-quick-terminal-namespace", "gtk-single-instance", "gtk-tabs-location",
    "gtk-titlebar", "gtk-titlebar-hide-when-maximized", "gtk-titlebar-style", "gtk-toolbar-style", "gtk-wide-tabs", "image-storage-limit",
    "initial-command", "initial-window", "input", "key-remap", "keybind", "language", "link-previews", "link-url", "linux-cgroup",
    "linux-cgroup-hard-fail", "linux-cgroup-memory-limit", "linux-cgroup-processes-limit", "macos-applescript", "macos-auto-secure-input",
    "macos-dock-drop-behavior", "macos-hidden", "macos-non-native-fullscreen", "macos-option-as-alt", "macos-secure-input-indication",
    "macos-shortcuts", "maximize", "mouse-hide-while-typing", "mouse-reporting", "mouse-scroll-multiplier", "mouse-shift-capture",
    "notify-on-command-finish", "notify-on-command-finish-action", "notify-on-command-finish-after", "osc-color-report-format",
    "quick-terminal-animation-duration", "quick-terminal-autohide", "quick-terminal-keyboard-interactivity", "quick-terminal-position",
    "quick-terminal-screen", "quick-terminal-space-behavior", "quit-after-last-window-closed", "quit-after-last-window-closed-delay",
    "right-click-action", "scroll-to-bottom", "scrollback-limit", "selection-clear-on-copy", "selection-clear-on-typing",
    "selection-word-chars", "shell-integration", "shell-integration-features", "split-inherit-working-directory", "split-preserve-zoom",
    "tab-inherit-working-directory", "term", "title", "title-report", "undo-timeout", "vt-kam-allowed", "wait-after-command",
    "window-height", "window-inherit-font-size", "window-inherit-working-directory", "window-new-tab-position", "window-position-x",
    "window-position-y", "window-save-state", "window-step-resize", "window-subtitle", "window-vsync", "window-width",
    "working-directory", "x11-instance-name",
];

pub fn find(key: &str) -> Option<&'static Opt> {
    LOOK.iter().find(|o| o.key == key)
}

pub fn is_repeatable(key: &str) -> bool {
    REPEATABLE.contains(&key)
}

/// A key Ghostty has that this catalog doesn't know yet: does its name say
/// it is about the look? (Those show in "Other"; the rest are ignored.)
pub fn looks_like_look(key: &str) -> bool {
    !NON_LOOK.contains(&key)
        && ["font-", "adjust-", "cursor-", "window-", "background-", "macos-icon", "macos-titlebar", "macos-window", "split-", "unfocused-", "selection-", "search-", "palette-", "custom-shader"]
            .iter()
            .any(|p| key.starts_with(p))
}

/// The docs of every key, from `+show-config --default --docs`: the comment
/// block above a key (keys without one share the block before them, as
/// `font-family-bold` shares `font-family`'s).
pub fn parse_docs(text: &str) -> std::collections::BTreeMap<String, String> {
    let mut docs = std::collections::BTreeMap::new();
    let mut block: Vec<&str> = Vec::new();
    let mut last = String::new();
    let mut fresh = false;
    for line in text.lines() {
        if let Some(c) = line.strip_prefix('#') {
            if !fresh {
                block.clear();
                fresh = true;
            }
            block.push(c.strip_prefix(' ').unwrap_or(c));
        } else if let Some((k, _)) = line.split_once(" = ") {
            if fresh {
                last = block.join("\n").trim().to_string();
                fresh = false;
            }
            docs.entry(k.trim().to_string()).or_insert_with(|| last.clone());
        }
    }
    docs
}
