use serde::Deserialize;

use crate::history::DEFAULT_MAX_SIZE;

static mut ICON_THEME: String = String::new();

#[derive(Deserialize, Default)]
pub struct ParsedConfig {
    window_width_percent: Option<u32>,
    window_height_percent: Option<u32>,
    entry_height: Option<u32>,
    list_item_height: Option<u32>,
    entry_font: Option<String>,
    list_font: Option<String>,
    list_empty_font: Option<String>,
    icon_theme: Option<String>,
    scroll_speed: Option<i32>,
    locale: Option<String>,
    scroll_bar_width: Option<u32>,
    history_entries: Option<usize>,
}

#[derive(Clone)]
pub struct Config {
    pub window_width_percent: u32,
    pub window_height_percent: u32,
    pub entry_height: u32,
    pub list_item_height: u32,
    pub entry_font: String,
    pub list_font: String,
    pub list_empty_font: String,
    pub scroll_speed: i32,
    pub locale: Option<String>,
    pub scroll_bar_width: u32,
    pub history_entries: usize,
}

impl Config {
    pub fn load() -> Self {
        let home = std::env::var("HOME").unwrap();
        let pathname = format!("{home}/.config/launcher.toml");
        let parsed = if let Ok(content) = std::fs::read_to_string(pathname) {
            toml::from_str(&content).unwrap_or_else(|error| {
                eprintln!("Config loading error: {error}");
                ParsedConfig::default()
            })
        } else {
            ParsedConfig::default()
        };
        unsafe {
            let theme_name = parsed.icon_theme.unwrap_or_else(|| "Papirus".to_string());
            ICON_THEME = find_icon_theme(theme_name);
        }
        Config {
            window_width_percent: parsed.window_width_percent.unwrap_or(50),
            window_height_percent: parsed.window_height_percent.unwrap_or(50),
            entry_height: parsed.entry_height.unwrap_or(48),
            list_item_height: parsed.list_item_height.unwrap_or(44),
            entry_font: parsed.entry_font.unwrap_or_else(|| "sans 24".to_string()),
            list_font: parsed.list_font.unwrap_or_else(|| "sans 20".to_string()),
            list_empty_font: parsed
                .list_empty_font
                .unwrap_or_else(|| "sans 48".to_string()),
            scroll_speed: parsed.scroll_speed.unwrap_or(10),
            locale: parsed.locale,
            scroll_bar_width: parsed.scroll_bar_width.unwrap_or(8),
            history_entries: parsed.history_entries.unwrap_or(DEFAULT_MAX_SIZE),
        }
    }
}

// FIXME: https://specifications.freedesktop.org/icon-theme-spec/latest/
fn find_icon_theme(name: String) -> String {
    let home = std::env::var("HOME").unwrap();
    let directories = [
        "/usr/share/icons".to_string(),
        format!("{home}/.local/share/icons"),
        format!("{home}/.icons"),
    ];
    for d in directories {
        let path = format!("{}/{}", d, name);
        if std::fs::metadata(&path).is_ok() {
            println!("Found icon theme: {path}");
            return path;
        }
    }
    panic!("Theme not found: {name}");
}

pub fn icon_search_path() -> &'static str {
    unsafe { ICON_THEME.as_str() }
}
