//! The config file (`$XDG_CONFIG_HOME/oxishot/config.toml`): where saves go
//! and the keyboard shortcuts. Missing entries fall back to [`DEFAULT`];
//! writing it back keeps the user's comments and layout.

use std::{fmt, fs, io, path::PathBuf};

use slint::platform::Key;
use toml_edit::{Array, DocumentMut, Item, Value};

use crate::Action;

/// Also what a new config file starts as.
pub const DEFAULT: &str = r#"# oxishot configuration. Edit it here or in the settings (the gear button);
# edits made here apply the next time oxishot starts or the settings open.

[save]
# Where screenshots are saved, and where the save dialog starts.
# "~" is your home directory; empty lets the dialog decide.
directory = ""
# true: Save writes straight into `directory` without asking.
skip_dialog = false

[keys]
# A shortcut, or a list of them, per action; "" leaves the action unbound.
# Modifiers: Ctrl, Shift, Alt. Named keys: Delete, Backspace, Tab, Enter,
# Space, Home, End, PageUp, PageDown, Insert, Up, Down, Left, Right, F1 to
# F12. Escape is not bindable: it always deselects and closes popups.
undo = "Ctrl+Z"
redo = "Ctrl+Y"
save = "Ctrl+S"
copy = "Ctrl+C"
select_all = "Ctrl+A"
delete = ["Delete", "Backspace"]
select = "V"
brush = "B"
text = "T"
rectangle = "R"
circle = "O"
arrow = "A"
blur = "U"
eyedropper = "I"
settings = ""
exit = ""
"#;

/// The actions shortcuts can trigger: (action, key in `[keys]`, name in the UI).
pub const BINDABLE: &[(Action, &str, &str)] = &[
    (Action::Undo, "undo", "Undo"),
    (Action::Redo, "redo", "Redo"),
    (Action::Save, "save", "Save"),
    (Action::Copy, "copy", "Copy"),
    (Action::SelectAll, "select_all", "Select all"),
    (Action::Delete, "delete", "Delete selection"),
    (Action::Select, "select", "Select tool"),
    (Action::Brush, "brush", "Brush"),
    (Action::Text, "text", "Text"),
    (Action::Rectangle, "rectangle", "Rectangle"),
    (Action::Circle, "circle", "Circle"),
    (Action::Arrow, "arrow", "Arrow"),
    (Action::Blur, "blur", "Blur"),
    (Action::Eyedropper, "eyedropper", "Eyedropper"),
    (Action::Settings, "settings", "Settings"),
    (Action::Exit, "exit", "Exit"),
];

/// Keys with a name; any other key is the character it types.
const NAMED: &[(Key, &str)] = &[
    (Key::Delete, "Delete"),
    (Key::Backspace, "Backspace"),
    (Key::Tab, "Tab"),
    (Key::Return, "Enter"),
    (Key::Space, "Space"),
    (Key::Home, "Home"),
    (Key::End, "End"),
    (Key::PageUp, "PageUp"),
    (Key::PageDown, "PageDown"),
    (Key::Insert, "Insert"),
    (Key::UpArrow, "Up"),
    (Key::DownArrow, "Down"),
    (Key::LeftArrow, "Left"),
    (Key::RightArrow, "Right"),
    (Key::F1, "F1"),
    (Key::F2, "F2"),
    (Key::F3, "F3"),
    (Key::F4, "F4"),
    (Key::F5, "F5"),
    (Key::F6, "F6"),
    (Key::F7, "F7"),
    (Key::F8, "F8"),
    (Key::F9, "F9"),
    (Key::F10, "F10"),
    (Key::F11, "F11"),
    (Key::F12, "F12"),
];

/// A key with modifiers. `key` is a name from [`NAMED`] or a lowercase character.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Combo {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub key: String,
}

impl Combo {
    /// "Ctrl+Shift+Z", "Delete", "Ctrl++"; modifier and key names ignore case.
    pub fn parse(s: &str) -> Option<Combo> {
        let s = s.trim();
        // the key itself may be "+"
        let (mods, key) = match s.strip_suffix('+') {
            Some(rest) if rest.is_empty() || rest.ends_with('+') => (rest, "+"),
            _ => s.rsplit_once('+').unwrap_or(("", s)),
        };
        let mut c = Combo { ctrl: false, alt: false, shift: false, key: key_name(key.trim())? };
        for m in mods.split('+').map(str::trim).filter(|m| !m.is_empty()) {
            match m.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => c.ctrl = true,
                "alt" => c.alt = true,
                "shift" => c.shift = true,
                _ => return None,
            }
        }
        Some(c)
    }

    /// A key press as Slint reports it; `None` for a lone modifier and other
    /// keys a shortcut can't use.
    pub fn from_event(text: &str, ctrl: bool, alt: bool, shift: bool) -> Option<Combo> {
        let mut chars = text.chars();
        let (Some(ch), None) = (chars.next(), chars.next()) else {
            return None;
        };
        let key = if let Some(&(_, name)) = NAMED.iter().find(|&&(k, _)| char::from(k) == ch) {
            name.to_string()
        } else if ch.is_control() || ('\u{F700}'..='\u{F8FF}').contains(&ch) {
            return None; // modifiers and other special keys
        } else {
            ch.to_lowercase().collect()
        };
        // a symbol typed with Shift ("!") already says so; letters don't
        let shift = shift && (key.chars().count() > 1 || ch.is_alphabetic());
        Some(Combo { ctrl, alt, shift, key })
    }
}

impl fmt::Display for Combo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (on, name) in [(self.ctrl, "Ctrl+"), (self.alt, "Alt+"), (self.shift, "Shift+")] {
            if on {
                f.write_str(name)?;
            }
        }
        if self.key.chars().count() == 1 {
            f.write_str(&self.key.to_uppercase())
        } else {
            f.write_str(&self.key)
        }
    }
}

/// The canonical form of a key written in the config.
fn key_name(s: &str) -> Option<String> {
    let alias = match s.to_ascii_lowercase().as_str() {
        "del" => "Delete",
        "return" => "Enter",
        _ => s,
    };
    if let Some(&(_, name)) = NAMED.iter().find(|(_, n)| n.eq_ignore_ascii_case(alias)) {
        return Some(name.to_string());
    }
    let mut chars = s.chars();
    match (chars.next(), chars.next()) {
        (Some(ch), None) if !ch.is_control() && !ch.is_whitespace() => Some(ch.to_lowercase().collect()),
        _ => None,
    }
}

/// Shortcuts written as one, a list, or none.
pub fn join(combos: &[Combo]) -> String {
    combos.iter().map(Combo::to_string).collect::<Vec<_>>().join(", ")
}

#[derive(Clone, Debug)]
pub struct Config {
    /// As written; see [`Config::save_dir`].
    pub directory: String,
    pub skip_dialog: bool,
    /// The shortcuts of each [`BINDABLE`] action, in its order.
    pub keys: Vec<Vec<Combo>>,
}

impl Default for Config {
    fn default() -> Config {
        let mut c = Config { directory: String::new(), skip_dialog: false, keys: vec![Vec::new(); BINDABLE.len()] };
        let doc = DEFAULT.parse().expect("the default config parses");
        let errors = c.overlay(&doc);
        debug_assert!(errors.is_empty(), "{errors:?}");
        c
    }
}

impl Config {
    pub fn path() -> Option<PathBuf> {
        let dir = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|d| !d.is_empty())
            .map(PathBuf::from)
            .or_else(|| home().map(|h| h.join(".config")))
            .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))?;
        Some(dir.join("oxishot").join("config.toml"))
    }

    /// The config file over the defaults, and what in it was wrong (and
    /// left at the default).
    pub fn load() -> (Config, Vec<String>) {
        let mut c = Config::default();
        let Some(path) = Config::path() else {
            return (c, vec!["no config directory (HOME is not set)".into()]);
        };
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return (c, Vec::new()),
            Err(e) => return (c, vec![format!("could not read {}: {e}", path.display())]),
        };
        let errors = match text.parse::<DocumentMut>() {
            Ok(doc) => c.overlay(&doc),
            Err(e) => vec![format!("{}: {e}", path.display())],
        };
        (c, errors)
    }

    fn overlay(&mut self, doc: &DocumentMut) -> Vec<String> {
        let mut errors = Vec::new();
        if let Some(save) = doc.get("save") {
            match save.get("directory") {
                None => {}
                Some(v) => match v.as_str() {
                    Some(s) => self.directory = s.to_string(),
                    None => errors.push("save.directory must be a string".into()),
                },
            }
            match save.get("skip_dialog") {
                None => {}
                Some(v) => match v.as_bool() {
                    Some(b) => self.skip_dialog = b,
                    None => errors.push("save.skip_dialog must be true or false".into()),
                },
            }
        }
        if let Some(keys) = doc.get("keys").and_then(Item::as_table_like) {
            for (name, item) in keys.iter() {
                let Some(i) = BINDABLE.iter().position(|&(_, n, _)| n == name) else {
                    errors.push(format!("keys.{name}: no such action"));
                    continue;
                };
                match read_combos(item) {
                    Ok(combos) => self.keys[i] = combos,
                    Err(e) => errors.push(format!("keys.{name}: {e}")),
                }
            }
        }
        errors
    }

    /// Writes this into the config file, keeping what else is in it.
    pub fn save(&self) -> Result<PathBuf, String> {
        let path = Config::path().ok_or("no config directory (HOME is not set)")?;
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => DEFAULT.to_string(),
            Err(e) => return Err(format!("could not read {}: {e}", path.display())),
        };
        let mut doc: DocumentMut = text.parse().map_err(|e| format!("{}: {e}", path.display()))?;
        set(&mut doc["save"]["directory"], self.directory.as_str().into());
        set(&mut doc["save"]["skip_dialog"], self.skip_dialog.into());
        for (&(_, name, _), combos) in BINDABLE.iter().zip(&self.keys) {
            let v = match combos.as_slice() {
                [] => "".into(),
                [one] => one.to_string().into(),
                many => Value::Array(many.iter().map(Combo::to_string).collect::<Array>()),
            };
            set(&mut doc["keys"][name], v);
        }
        write(&path, &doc.to_string())?;
        Ok(path)
    }

    /// Creates the file with the defaults if there's none yet.
    pub fn ensure_file() -> Result<PathBuf, String> {
        let path = Config::path().ok_or("no config directory (HOME is not set)")?;
        if !path.exists() {
            write(&path, DEFAULT)?;
        }
        Ok(path)
    }

    /// The save directory, `~` expanded; `None` if unset.
    pub fn save_dir(&self) -> Option<PathBuf> {
        expand_dir(&self.directory)
    }

    pub fn action_for(&self, combo: &Combo) -> Option<Action> {
        let i = self.keys.iter().position(|cs| cs.contains(combo))?;
        Some(BINDABLE[i].0)
    }

    /// The first shortcut of `action`, for tooltips; empty if unbound.
    pub fn label(&self, action: Action) -> String {
        BINDABLE
            .iter()
            .position(|&(a, _, _)| a == action)
            .and_then(|i| self.keys[i].first())
            .map_or_else(String::new, Combo::to_string)
    }
}

/// A directory as written in the config, `~` expanded; `None` if empty.
pub fn expand_dir(dir: &str) -> Option<PathBuf> {
    let d = dir.trim();
    if d.is_empty() {
        return None;
    }
    match d.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with(['/', '\\']) => {
            Some(home()?.join(rest.trim_start_matches(['/', '\\'])))
        }
        _ => Some(PathBuf::from(d)),
    }
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}

fn read_combos(item: &Item) -> Result<Vec<Combo>, String> {
    let one = |s: &str| Combo::parse(s).ok_or_else(|| format!("\"{s}\" is not a shortcut"));
    if let Some(s) = item.as_str() {
        return if s.trim().is_empty() { Ok(Vec::new()) } else { Ok(vec![one(s)?]) };
    }
    let arr = item.as_array().ok_or("must be a string or a list of strings")?;
    arr.iter()
        .map(|v| v.as_str().ok_or_else(|| "must be a string or a list of strings".to_string()).and_then(one))
        .collect()
}

/// Replaces the value at `item`, keeping the comments around it.
fn set(item: &mut Item, v: Value) {
    let decor = item.as_value().map(|old| old.decor().clone());
    let mut v = v;
    if let Some(d) = decor {
        *v.decor_mut() = d;
    }
    *item = Item::Value(v);
}

fn write(path: &std::path::Path, text: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    }
    fs::write(path, text).map_err(|e| format!("could not write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn combo(s: &str) -> Combo {
        Combo::parse(s).unwrap()
    }

    #[test]
    fn parses_shortcuts() {
        assert_eq!(combo("ctrl+shift+z").to_string(), "Ctrl+Shift+Z");
        assert!(Combo::parse("Escape").is_none());
        assert_eq!(combo("Ctrl++").key, "+");
        assert_eq!(combo("+").key, "+");
        assert!(Combo::parse("Hyper+X").is_none());
        assert!(Combo::parse("Ctrl+").is_none());
        assert!(Combo::parse("Banana").is_none());
    }

    #[test]
    fn events_match_config() {
        let ev = |t: &str, ctrl, shift| Combo::from_event(t, ctrl, false, shift);
        assert_eq!(ev("Z", true, true), Some(combo("Ctrl+Shift+Z")));
        assert_eq!(ev("v", false, false), Some(combo("V")));
        assert_eq!(ev("!", false, true), Some(combo("!")));
        assert_eq!(ev(&char::from(Key::Delete).to_string(), false, false), Some(combo("Delete")));
        assert_eq!(ev(&char::from(Key::Shift).to_string(), false, true), None);
        assert_eq!(ev(&char::from(Key::Escape).to_string(), false, false), None);
    }

    #[test]
    fn defaults_and_overlay() {
        let c = Config::default();
        assert_eq!(c.action_for(&combo("Backspace")), Some(Action::Delete));
        assert_eq!(c.label(Action::Undo), "Ctrl+Z");
        assert_eq!(c.label(Action::Exit), "");

        let mut c = Config::default();
        let doc = "[keys]\nundo = \"Ctrl+U\"\nnope = \"X\"\nredo = 3\n".parse().unwrap();
        let errors = c.overlay(&doc);
        assert_eq!(errors.len(), 2);
        assert_eq!(c.label(Action::Undo), "Ctrl+U");
        assert_eq!(c.label(Action::Redo), "Ctrl+Y");
    }

    #[test]
    fn saving_keeps_comments() {
        let mut doc: DocumentMut = "[keys]\nundo = \"Ctrl+Z\" # mine\n".parse().unwrap();
        set(&mut doc["keys"]["undo"], "Ctrl+U".into());
        assert_eq!(doc.to_string(), "[keys]\nundo = \"Ctrl+U\" # mine\n");
    }
}
