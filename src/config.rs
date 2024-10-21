use crate::{
    content::{ContentOptions, UrlMode},
    history::DEFAULT_MAX_SIZE,
    icon_theme::IconRegistry,
    units::user_currency,
};
use serde::Deserialize;
use std::cell::RefCell;

thread_local! {
    pub static ICON_THEME: RefCell<IconRegistry> = Default::default();
}

#[derive(Deserialize, Default)]
pub struct ParsedConfig {
    window_width_percent: Option<u32>,
    window_height_percent: Option<u32>,
    entry_height: Option<u32>,
    list_item_height: Option<u32>,
    entry_font: Option<String>,
    list_font: Option<String>,
    list_empty_font: Option<String>,
    smart_content_font: Option<String>,
    icon_theme: Option<String>,
    scroll_speed: Option<i32>,
    locale: Option<String>,
    scroll_bar_width: Option<u32>,
    history_entries: Option<usize>,
    default_currency: Option<String>,
    smart_content_urls: Option<String>,
    smart_content_dynamic_conversions: Option<bool>,
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
    pub smart_content_font: String,
    pub scroll_speed: i32,
    pub locale: Option<String>,
    pub scroll_bar_width: u32,
    pub history_entries: usize,
    pub default_currency: String,
    pub smart_content_options: ContentOptions,
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
        let theme_name = parsed.icon_theme.as_deref().unwrap_or("Papirus");
        ICON_THEME.with_borrow_mut(|t| *t = IconRegistry::new(theme_name).unwrap());
        let url_mode = match parsed.smart_content_urls.as_deref() {
            Some("none") => UrlMode::None,
            Some("http") => UrlMode::Http,
            Some("all") | Some("loose") | None => UrlMode::Loose,
            Some(x) => {
                eprintln!("Invalid URL mode: {x}");
                UrlMode::Loose
            }
        };
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
            smart_content_font: parsed
                .smart_content_font
                .unwrap_or_else(|| "sans 32".to_string()),
            scroll_speed: parsed.scroll_speed.unwrap_or(10),
            locale: parsed.locale,
            scroll_bar_width: parsed.scroll_bar_width.unwrap_or(8),
            history_entries: parsed.history_entries.unwrap_or(DEFAULT_MAX_SIZE),
            default_currency: parsed
                .default_currency
                .unwrap_or_else(|| user_currency())
                .to_lowercase(),
            smart_content_options: ContentOptions {
                dynamic_conversions: parsed.smart_content_dynamic_conversions.unwrap_or(true),
                url_mode,
            },
        }
    }
}
