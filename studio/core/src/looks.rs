//! Saved looks: named clusters of settings you can press to get back to.
//! `looks.json` in the config folder (committed, so a saved look survives
//! anything): `{ "editor": [ { name, protected, look } ], "notes": [ … ] }`.
//! The built-in presets (`presets()`) are never written; they and the entries
//! marked `protected` can't be renamed, overwritten or deleted from Studio.
use crate::look::Look;
use crate::model::Paths;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Saved<T> {
    pub name: String,
    #[serde(default)]
    pub protected: bool,
    pub look: T,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Looks {
    pub editor: Vec<Saved<Look>>,
    /// Notes looks (core::notes), kept as JSON so an older Studio keeps them intact.
    pub notes: Vec<Saved<serde_json::Value>>,
    /// Terminal looks (core::ghostty, the Terminal page).
    pub terminal: Vec<Saved<crate::ghostty::TermLook>>,
}

pub fn path(paths: &Paths) -> PathBuf {
    paths.config.join("looks.json")
}

pub fn read(paths: &Paths) -> Looks {
    std::fs::read_to_string(path(paths)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn write(paths: &Paths, looks: &Looks) -> std::io::Result<()> {
    let p = path(paths);
    let tmp = p.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(looks).map_err(std::io::Error::other)? + "\n";
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, &p)
}

/// The editor presets that are always there: the default and stock Alabaster.
pub fn presets() -> Vec<Saved<Look>> {
    let mut alabaster = Look::default();
    alabaster.palette = "alabaster".into();
    vec![
        Saved { name: "Slate (default)".into(), protected: true, look: Look::default() },
        Saved { name: "Alabaster (stock)".into(), protected: true, look: alabaster },
    ]
}

/// A look as it is saved: the settings plus the colors of its own palette only.
pub fn snapshot(look: &Look) -> Look {
    let mut l = look.clone();
    l.colors.retain(|p, _| *p == look.palette);
    l
}

/// Put a saved look back: its settings, and its palette's colors exactly as
/// saved (the other palettes' color edits stay).
pub fn restore(current: &Look, saved: &Look) -> Look {
    let mut l = saved.clone();
    l.colors = current.colors.clone();
    match saved.colors.get(&saved.palette) {
        Some(c) => {
            l.colors.insert(saved.palette.clone(), c.clone());
        }
        None => {
            l.colors.remove(&saved.palette);
        }
    }
    l
}

/// Is `current` the same as the saved look (for marking the button)?
pub fn matches(current: &Look, saved: &Look) -> bool {
    snapshot(current) == snapshot(saved)
}

impl Looks {
    /// Every editor look to show: the presets, then the saved ones.
    pub fn editor_all(&self) -> Vec<Saved<Look>> {
        let mut v = presets();
        v.extend(self.editor.iter().cloned());
        v
    }

    fn name_taken(&self, name: &str) -> bool {
        presets().iter().any(|p| p.name == name) || self.editor.iter().any(|s| s.name == name)
    }

    /// Save `look` under `name`: a new entry, or over an unprotected one of
    /// that name. Err with the reason when the name is empty or protected.
    pub fn save_editor(&mut self, name: &str, look: &Look) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Give the look a name first.".into());
        }
        if presets().iter().any(|p| p.name == name) {
            return Err(format!("“{name}” is a preset; pick another name."));
        }
        match self.editor.iter_mut().find(|s| s.name == name) {
            Some(s) if s.protected => Err(format!("“{name}” is protected; pick another name.")),
            Some(s) => {
                s.look = snapshot(look);
                Ok(())
            }
            None => {
                self.editor.push(Saved { name: name.into(), protected: false, look: snapshot(look) });
                Ok(())
            }
        }
    }

    pub fn rename_editor(&mut self, from: &str, to: &str) -> Result<(), String> {
        let to = to.trim();
        if to.is_empty() {
            return Err("Type the new name first.".into());
        }
        if to != from && self.name_taken(to) {
            return Err(format!("There is already a look called “{to}”."));
        }
        match self.editor.iter_mut().find(|s| s.name == from) {
            Some(s) if s.protected => Err(format!("“{from}” is protected.")),
            Some(s) => {
                s.name = to.into();
                Ok(())
            }
            None => Err(format!("“{from}” is a preset; presets keep their names.")),
        }
    }

    pub fn delete_editor(&mut self, name: &str) -> Result<(), String> {
        match self.editor.iter().position(|s| s.name == name) {
            Some(i) if self.editor[i].protected => Err(format!("“{name}” is protected.")),
            Some(i) => {
                self.editor.remove(i);
                Ok(())
            }
            None => Err(format!("“{name}” is a preset; presets can't be deleted.")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_restore_rename_delete() {
        let mut looks = Looks::default();
        let mut l = Look::default();
        l.set("palette", "alabaster");
        l.set_color("math", 0xc41982);
        l.set("palette", "slate");
        l.set_color("math", 0x111111);
        l.set("palette", "alabaster");
        looks.save_editor("Mine", &l).unwrap();
        assert_eq!(looks.editor[0].look.colors.len(), 1, "only its own palette's colors are saved");
        // change everything, then restore
        let mut now = l.clone();
        now.reset_colors();
        now.set("sections", "band");
        let back = restore(&now, &looks.editor[0].look);
        assert_eq!(back.sections, "rule");
        assert_eq!(back.color("math"), 0xc41982);
        assert_eq!(back.colors["slate"]["math"], "#111111", "other palettes' edits stay");
        assert!(matches(&back, &looks.editor[0].look));
        // presets and protected looks are safe
        assert!(looks.save_editor("Slate (default)", &l).is_err());
        looks.editor[0].protected = true;
        assert!(looks.save_editor("Mine", &now).is_err());
        assert!(looks.delete_editor("Mine").is_err());
        assert!(looks.rename_editor("Mine", "Other").is_err());
        looks.editor[0].protected = false;
        looks.rename_editor("Mine", "Other").unwrap();
        assert!(looks.rename_editor("Other", "Alabaster (stock)").is_err());
        looks.delete_editor("Other").unwrap();
        assert!(looks.editor.is_empty());
        assert!(looks.delete_editor("Slate (default)").is_err());
    }

    #[test]
    fn restoring_a_look_without_color_edits_clears_them() {
        let mut now = Look::default();
        now.set_color("math", 0xff0000);
        let back = restore(&now, &presets()[0].look);
        assert!(back.colors.is_empty());
    }
}

// ---------------------------------------------------------------------------
// notes looks (core::notes), the same rules

/// The notes presets: the theme's own look (Charter, its colors and boxes).
pub fn notes_presets() -> Vec<Saved<crate::notes::NotesLook>> {
    vec![Saved { name: "Theme default (Charter)".into(), protected: true, look: crate::notes::NotesLook::default() }]
}

impl Looks {
    fn notes_typed(&self) -> Vec<Saved<crate::notes::NotesLook>> {
        self.notes
            .iter()
            .map(|s| Saved {
                name: s.name.clone(),
                protected: s.protected,
                look: crate::notes::parse(&s.look.to_string()),
            })
            .collect()
    }

    /// Every notes look to show: the presets, then the saved ones.
    pub fn notes_all(&self) -> Vec<Saved<crate::notes::NotesLook>> {
        let mut v = notes_presets();
        v.extend(self.notes_typed());
        v
    }

    pub fn save_notes(&mut self, name: &str, look: &crate::notes::NotesLook) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Give the look a name first.".into());
        }
        if notes_presets().iter().any(|p| p.name == name) {
            return Err(format!("“{name}” is a preset; pick another name."));
        }
        let value = serde_json::to_value(look).map_err(|e| e.to_string())?;
        match self.notes.iter_mut().find(|s| s.name == name) {
            Some(s) if s.protected => Err(format!("“{name}” is protected; pick another name.")),
            Some(s) => {
                s.look = value;
                Ok(())
            }
            None => {
                self.notes.push(Saved { name: name.into(), protected: false, look: value });
                Ok(())
            }
        }
    }

    pub fn rename_notes(&mut self, from: &str, to: &str) -> Result<(), String> {
        let to = to.trim();
        if to.is_empty() {
            return Err("Type the new name first.".into());
        }
        if to != from && (notes_presets().iter().any(|p| p.name == to) || self.notes.iter().any(|s| s.name == to)) {
            return Err(format!("There is already a look called “{to}”."));
        }
        match self.notes.iter_mut().find(|s| s.name == from) {
            Some(s) if s.protected => Err(format!("“{from}” is protected.")),
            Some(s) => {
                s.name = to.into();
                Ok(())
            }
            None => Err(format!("“{from}” is a preset; presets keep their names.")),
        }
    }

    pub fn delete_notes(&mut self, name: &str) -> Result<(), String> {
        match self.notes.iter().position(|s| s.name == name) {
            Some(i) if self.notes[i].protected => Err(format!("“{name}” is protected.")),
            Some(i) => {
                self.notes.remove(i);
                Ok(())
            }
            None => Err(format!("“{name}” is a preset; presets can't be deleted.")),
        }
    }
}

#[cfg(test)]
mod notes_tests {
    use super::*;

    #[test]
    fn notes_looks() {
        let mut looks = Looks::default();
        let mut n = crate::notes::NotesLook::default();
        n.set("font", "palatino");
        looks.save_notes("Pala", &n).unwrap();
        assert_eq!(looks.notes_all()[1].look.font, "palatino");
        assert!(looks.save_notes("Theme default (Charter)", &n).is_err());
        looks.rename_notes("Pala", "Book").unwrap();
        looks.delete_notes("Book").unwrap();
        assert!(looks.notes.is_empty());
    }
}
