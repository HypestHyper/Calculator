//! Application constants, file locations and the settings store.
//!
//! The files are the same ones the Python version (1.x) used, in the same
//! format, so settings, window sizes and history carry over.

use std::cell::RefCell;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use serde::{Deserialize, Serialize};

pub const APP_ID: &str = "io.github.HypestHyper.Calculator";
pub const APP_NAME: &str = "Calculator";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn xdg(var: &str, fallback: &str) -> PathBuf {
    match std::env::var_os(var).map(PathBuf::from) {
        Some(path) if path.is_absolute() => path,
        _ => home().join(fallback),
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

pub fn config_dir() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config").join(APP_ID)
}

pub fn state_dir() -> PathBuf {
    xdg("XDG_STATE_HOME", ".local/state").join(APP_ID)
}

/// Where bundled data (fonts) may live: the source tree first, then installs.
pub fn data_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("data")];
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
    {
        dirs.push(exe_dir.join("../share").join(APP_ID));
    }
    let xdg_data =
        std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    let user = xdg("XDG_DATA_HOME", ".local/share");
    for base in std::iter::once(user).chain(
        xdg_data
            .split(':')
            .filter(|s| !s.is_empty())
            .map(PathBuf::from),
    ) {
        dirs.push(base.join(APP_ID));
    }
    dirs
}

/// Write a file through a temporary file, so a crash never leaves half a file.
fn write_atomic(path: &Path, contents: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, contents)?;
    fs::rename(&tmp, path)
}

// --------------------------------------------------------------------------
// Settings
// --------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Str(String),
}

enum Rule {
    Choice(&'static [&'static str]),
    Bool,
    /// -1 (auto) to 12 decimal places.
    Decimals,
    HexColor,
}

const PRESETS: &[&str] = &[
    "lavender",
    "ocean",
    "forest",
    "teal",
    "rose",
    "sunset",
    "honey",
    "candy",
    "graphite",
    "catppuccin",
    "nord",
    "gruvbox",
    "tokyo_night",
    "rose_pine",
    "dracula",
];

/// Every setting, in order, with its default.
/// Settings this version does not use yet are still read and written back.
fn schema() -> Vec<(&'static str, Value, Rule)> {
    use Rule::{Choice, Decimals, HexColor};
    use Value::{Bool, Int};
    let s = |v: &str| Value::Str(v.to_string());
    vec![
        ("theme", s("system"), Choice(&["system", "light", "dark"])),
        (
            "color_source",
            s("desktop"),
            Choice(&["desktop", "preset", "custom", "palette"]),
        ),
        ("preset", s("lavender"), Choice(PRESETS)),
        ("custom_accent", s("#6750A4"), HexColor),
        (
            "palette_style",
            s("tonal_spot"),
            Choice(&[
                "tonal_spot",
                "vibrant",
                "expressive",
                "fidelity",
                "content",
                "neutral",
                "monochrome",
                "rainbow",
                "fruit_salad",
            ]),
        ),
        (
            "contrast",
            s("system"),
            Choice(&["system", "standard", "medium", "high"]),
        ),
        (
            "text_size",
            s("medium"),
            Choice(&["small", "medium", "large"]),
        ),
        (
            "button_size",
            s("standard"),
            Choice(&["compact", "standard", "large"]),
        ),
        ("live_preview", Bool(true), Rule::Bool),
        ("digit_grouping", Bool(true), Rule::Bool),
        ("decimals", Int(-1), Decimals),
        ("decimal_separator", s("point"), Choice(&["point", "comma"])),
        ("angle_unit", s("deg"), Choice(&["deg", "rad"])),
        (
            "motion",
            s("system"),
            Choice(&["system", "expressive", "standard", "off"]),
        ),
        ("save_history", Bool(true), Rule::Bool),
        ("remember_size", Bool(true), Rule::Bool),
        ("themed_icon", Bool(true), Rule::Bool),
        ("scientific", Bool(false), Rule::Bool),
    ]
}

fn valid(rule: &Rule, value: &Value) -> bool {
    match (rule, value) {
        (Rule::Choice(choices), Value::Str(s)) => choices.contains(&s.as_str()),
        (Rule::Bool, Value::Bool(_)) => true,
        (Rule::Decimals, Value::Int(n)) => (-1..=12).contains(n),
        (Rule::HexColor, Value::Str(s)) => {
            let hex = s.trim_start_matches('#');
            hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit())
        }
        _ => false,
    }
}

fn toml_value(value: &Value) -> String {
    match value {
        Value::Bool(b) => b.to_string(),
        Value::Int(n) => n.to_string(),
        // A JSON string is a valid TOML basic string for these values.
        Value::Str(s) => serde_json::to_string(s).unwrap(),
    }
}

type Listener = Box<dyn Fn(&str)>;

/// Small observable settings store backed by settings.toml.
pub struct Settings {
    path: PathBuf,
    values: RefCell<Vec<(&'static str, Value)>>,
    listeners: RefCell<Vec<Rc<Listener>>>,
}

impl Settings {
    pub fn new(path: PathBuf) -> Rc<Self> {
        let values = schema()
            .into_iter()
            .map(|(key, default, _)| (key, default))
            .collect();
        let settings = Rc::new(Settings {
            path,
            values: RefCell::new(values),
            listeners: RefCell::default(),
        });
        settings.load();
        settings
    }

    pub fn load_default() -> Rc<Self> {
        Self::new(config_dir().join("settings.toml"))
    }

    fn load(&self) {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return,
            Err(err) => {
                eprintln!("calculator: ignoring unreadable settings: {err}");
                return;
            }
        };
        let table: toml::Table = match text.parse() {
            Ok(table) => table,
            Err(err) => {
                eprintln!("calculator: ignoring unreadable settings: {err}");
                return;
            }
        };
        let rules = schema();
        let mut values = self.values.borrow_mut();
        for (key, value) in values.iter_mut() {
            let Some(raw) = table.get(*key) else { continue };
            let parsed = match raw {
                toml::Value::Boolean(b) => Value::Bool(*b),
                toml::Value::Integer(n) => Value::Int(*n),
                toml::Value::String(s) => Value::Str(s.clone()),
                _ => continue,
            };
            let rule = &rules.iter().find(|(k, _, _)| k == key).unwrap().2;
            if valid(rule, &parsed) {
                *value = parsed;
            }
        }
    }

    fn save(&self) {
        let mut text =
            String::from("# Calculator settings. Edited from Preferences; safe to edit by hand.\n");
        for (key, value) in self.values.borrow().iter() {
            text.push_str(&format!("{key} = {}\n", toml_value(value)));
        }
        if let Err(err) = write_atomic(&self.path, &text) {
            eprintln!("calculator: could not save settings: {err}");
        }
    }

    pub fn get(&self, key: &str) -> Value {
        let values = self.values.borrow();
        values
            .iter()
            .find(|(k, _)| *k == key)
            .unwrap_or_else(|| panic!("unknown setting {key}"))
            .1
            .clone()
    }

    pub fn bool(&self, key: &str) -> bool {
        match self.get(key) {
            Value::Bool(b) => b,
            other => panic!("{key} is not a bool: {other:?}"),
        }
    }

    pub fn int(&self, key: &str) -> i64 {
        match self.get(key) {
            Value::Int(n) => n,
            other => panic!("{key} is not an int: {other:?}"),
        }
    }

    pub fn str(&self, key: &str) -> String {
        match self.get(key) {
            Value::Str(s) => s,
            other => panic!("{key} is not a string: {other:?}"),
        }
    }

    /// Change a setting, save, and tell the listeners. Invalid values are refused.
    pub fn set(&self, key: &str, value: Value) -> Result<(), String> {
        let rules = schema();
        let Some((_, _, rule)) = rules.iter().find(|(k, _, _)| *k == key) else {
            return Err(format!("unknown setting {key}"));
        };
        if !valid(rule, &value) {
            return Err(format!("invalid value for {key}: {value:?}"));
        }
        {
            let mut values = self.values.borrow_mut();
            let slot = &mut values.iter_mut().find(|(k, _)| *k == key).unwrap().1;
            if *slot == value {
                return Ok(());
            }
            *slot = value;
        }
        self.save();
        let listeners: Vec<_> = self.listeners.borrow().clone();
        for listener in listeners {
            listener(key);
        }
        Ok(())
    }

    pub fn connect(&self, callback: impl Fn(&str) + 'static) {
        self.listeners
            .borrow_mut()
            .push(Rc::new(Box::new(callback)));
    }
}

// --------------------------------------------------------------------------
// Window state and history (JSON in the state dir)
// --------------------------------------------------------------------------

/// The unit converter's state: whether it is open, the category, and the
/// units last used in each category. Kept with the window sizes.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ConverterState {
    #[serde(skip)]
    path: PathBuf,
    #[serde(default)]
    pub open: bool,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub units: std::collections::BTreeMap<String, (String, String)>,
}

impl ConverterState {
    pub fn load_default() -> Self {
        Self::load(state_dir().join("converter.json"))
    }

    pub fn load(path: PathBuf) -> Self {
        let mut state: ConverterState = fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        state.path = path;
        state
    }

    pub fn save(&self) {
        let text = serde_json::to_string_pretty(self).unwrap();
        if let Err(err) = write_atomic(&self.path, &text) {
            eprintln!("calculator: could not write {}: {err}", self.path.display());
        }
    }
}

/// By default, the scientific window is this much wider than the standard
/// one, at the same height; remembered sizes use the panel's real width.
pub const SCIENTIFIC_WIDTH_RATIO: f64 = 1.8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    Standard,
    Scientific,
}

impl Mode {
    pub fn key(self) -> &'static str {
        match self {
            Mode::Standard => "standard",
            Mode::Scientific => "scientific",
        }
    }

    pub fn default_size(self) -> (i32, i32) {
        match self {
            Mode::Standard => (220, 330),
            Mode::Scientific => ((220.0 * SCIENTIFIC_WIDTH_RATIO).round() as i32, 330),
        }
    }
}

pub struct WindowState {
    path: PathBuf,
    standard: (i32, i32),
    scientific: (i32, i32),
    /// Where the window's centre was, and the method that measured it
    /// (see placement.rs).
    pub center: Option<(i32, i32)>,
    pub placed_by: Option<String>,
}

impl WindowState {
    pub fn load_default() -> Self {
        Self::load(state_dir().join("window.json"))
    }

    pub fn load(path: PathBuf) -> Self {
        let mut state = WindowState {
            path,
            standard: Mode::Standard.default_size(),
            scientific: Mode::Scientific.default_size(),
            center: None,
            placed_by: None,
        };
        let data: serde_json::Value = fs::read_to_string(&state.path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or(serde_json::Value::Null);
        for mode in [Mode::Standard, Mode::Scientific] {
            let size = data
                .get(mode.key())
                .and_then(|v| v.as_array())
                .and_then(|a| match a.as_slice() {
                    [w, h] => Some((w.as_i64()?, h.as_i64()?)),
                    _ => None,
                });
            if let Some((w, h)) = size.filter(|&(w, h)| w > 100 && h > 100) {
                state.set(mode, (w as i32, h as i32));
            }
        }
        state.center =
            data.get("center")
                .and_then(|v| v.as_array())
                .and_then(|a| match a.as_slice() {
                    [x, y] => Some((x.as_i64()? as i32, y.as_i64()? as i32)),
                    _ => None,
                });
        state.placed_by = data
            .get("placed_by")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        state
    }

    fn set(&mut self, mode: Mode, size: (i32, i32)) {
        match mode {
            Mode::Standard => self.standard = size,
            Mode::Scientific => self.scientific = size,
        }
    }

    pub fn size_for(&self, mode: Mode) -> (i32, i32) {
        match mode {
            Mode::Standard => self.standard,
            Mode::Scientific => self.scientific,
        }
    }

    pub fn reset(&mut self) {
        self.standard = Mode::Standard.default_size();
        self.scientific = Mode::Scientific.default_size();
    }

    /// Remember the size for `mode`, and give the other mode the same height
    /// and a width `panel` wider (scientific) or narrower (standard): the
    /// width of the scientific keys at this size, so switching keeps the size
    /// the user chose and the keys fill both windows alike.
    pub fn remember(&mut self, mode: Mode, width: i32, height: i32, panel: i32) {
        if width > 100 && height > 100 {
            self.set(mode, (width, height));
            let (other, other_width) = match mode {
                Mode::Standard => (Mode::Scientific, width + panel),
                Mode::Scientific => (Mode::Standard, (width - panel).max(101)),
            };
            self.set(other, (other_width, height));
        }
    }

    pub fn save(&self) {
        let mut data = serde_json::json!({
            "standard": [self.standard.0, self.standard.1],
            "scientific": [self.scientific.0, self.scientific.1],
        });
        if let (Some((x, y)), Some(method)) = (self.center, &self.placed_by) {
            data["center"] = serde_json::json!([x, y]);
            data["placed_by"] = serde_json::json!(method);
        }
        let text = serde_json::to_string_pretty(&data).unwrap();
        if let Err(err) = write_atomic(&self.path, &text) {
            eprintln!("calculator: could not write {}: {err}", self.path.display());
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryItem {
    pub expr: String,
    pub result: String,
    pub value: f64,
}

pub struct History {
    path: PathBuf,
    enabled: RefCell<bool>,
    /// Newest first.
    items: RefCell<Vec<HistoryItem>>,
    listeners: RefCell<Vec<Rc<dyn Fn()>>>,
}

impl History {
    pub const LIMIT: usize = 100;

    pub fn load_default(enabled: bool) -> Rc<Self> {
        Self::load(state_dir().join("history.json"), enabled)
    }

    pub fn load(path: PathBuf, enabled: bool) -> Rc<Self> {
        let mut items = Vec::new();
        if enabled {
            // Skip entries that are not well-formed rather than losing the file.
            let data: Vec<serde_json::Value> = fs::read_to_string(&path)
                .ok()
                .and_then(|text| serde_json::from_str(&text).ok())
                .unwrap_or_default();
            items = data
                .into_iter()
                .filter_map(|v| serde_json::from_value(v).ok())
                .take(Self::LIMIT)
                .collect();
        }
        Rc::new(History {
            path,
            enabled: RefCell::new(enabled),
            items: RefCell::new(items),
            listeners: RefCell::default(),
        })
    }

    pub fn items(&self) -> Vec<HistoryItem> {
        self.items.borrow().clone()
    }

    pub fn connect(&self, callback: impl Fn() + 'static) {
        self.listeners.borrow_mut().push(Rc::new(callback));
    }

    fn changed(&self) {
        if *self.enabled.borrow() {
            // Indentation differs from the Python version's; both read either.
            let text = serde_json::to_string_pretty(&*self.items.borrow()).unwrap();
            if let Err(err) = write_atomic(&self.path, &text) {
                eprintln!("calculator: could not write {}: {err}", self.path.display());
            }
        }
        let listeners: Vec<_> = self.listeners.borrow().clone();
        for listener in listeners {
            listener();
        }
    }

    pub fn add(&self, expr: &str, result: &str, value: f64) {
        {
            let mut items = self.items.borrow_mut();
            if items.first().is_some_and(|first| first.expr == expr) {
                return;
            }
            items.insert(
                0,
                HistoryItem {
                    expr: expr.into(),
                    result: result.into(),
                    value,
                },
            );
            items.truncate(Self::LIMIT);
        }
        self.changed();
    }

    pub fn clear(&self) {
        self.items.borrow_mut().clear();
        self.changed();
        if !*self.enabled.borrow() {
            let _ = fs::remove_file(&self.path);
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        *self.enabled.borrow_mut() = enabled;
        if enabled {
            self.changed();
        } else {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("calculator-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn settings_round_trip_keeps_valid_values_and_drops_bad_ones() {
        let dir = temp("settings");
        let path = dir.join("settings.toml");
        fs::write(
            &path,
            "theme = \"dark\"\ndecimals = 99\nscientific = true\nunknown = 1\npreset = \"nope\"\n",
        )
        .unwrap();
        let settings = Settings::new(path.clone());
        assert_eq!(settings.str("theme"), "dark");
        assert_eq!(settings.int("decimals"), -1);
        assert!(settings.bool("scientific"));
        assert_eq!(settings.str("preset"), "lavender");
        assert!(settings.set("decimals", Value::Int(13)).is_err());
        settings
            .set("decimal_separator", Value::Str("comma".into()))
            .unwrap();
        let again = Settings::new(path.clone());
        assert_eq!(again.str("decimal_separator"), "comma");
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("# Calculator settings."));
        assert!(text.contains("theme = \"dark\"\n"));
        assert!(!text.contains("unknown"));
    }

    #[test]
    fn history_reads_python_files_and_skips_bad_entries() {
        let dir = temp("history");
        let path = dir.join("history.json");
        fs::write(
            &path,
            r#"[{"expr": "1+1", "result": "2", "value": 2.0}, {"expr": "x"}, 5]"#,
        )
        .unwrap();
        let history = History::load(path.clone(), true);
        assert_eq!(history.items().len(), 1);
        history.add("2×3", "6", 6.0);
        history.add("2×3", "6", 6.0);
        assert_eq!(history.items()[0].expr, "2×3");
        assert_eq!(History::load(path, true).items().len(), 2);
    }

    #[test]
    fn window_state_keeps_the_centre() {
        let path = temp("centre").join("window.json");
        let mut state = WindowState::load(path.clone());
        assert_eq!(state.center, None);
        state.center = Some((1610, 365));
        state.placed_by = Some("sway".into());
        state.save();
        let state = WindowState::load(path);
        assert_eq!(state.center, Some((1610, 365)));
        assert_eq!(state.placed_by.as_deref(), Some("sway"));
    }

    #[test]
    fn remembering_one_mode_sizes_the_other() {
        let mut state = WindowState::load(temp("ratio").join("window.json"));
        state.remember(Mode::Standard, 250, 400, 200);
        assert_eq!(state.size_for(Mode::Scientific), (450, 400));
        state.remember(Mode::Scientific, 900, 500, 400);
        assert_eq!(state.size_for(Mode::Standard), (500, 500));
    }

    #[test]
    fn converter_state_round_trips() {
        let path = temp("converter").join("converter.json");
        let mut state = ConverterState::load(path.clone());
        assert!(!state.open && state.units.is_empty());
        state.open = true;
        state.category = "length".into();
        state
            .units
            .insert("length".into(), ("mm".into(), "in".into()));
        state.save();
        let state = ConverterState::load(path);
        assert!(state.open);
        assert_eq!(state.units["length"], ("mm".into(), "in".into()));
    }

    #[test]
    fn window_state_ignores_tiny_sizes() {
        let dir = temp("window");
        let path = dir.join("window.json");
        fs::write(&path, r#"{"standard": [50, 50], "scientific": [800, 700]}"#).unwrap();
        let state = WindowState::load(path);
        assert_eq!(state.size_for(Mode::Standard), (220, 330));
        assert_eq!(state.size_for(Mode::Scientific), (800, 700));
    }
}
