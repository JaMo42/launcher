use serde::Deserialize;

#[derive(Deserialize, Default)]
pub struct ParsedConfig {
  window_width_percent: Option<u32>,
  window_height_percent: Option<u32>,
  entry_height: Option<u32>,
  list_item_height: Option<u32>,
  entry_font: Option<String>,
  list_font: Option<String>,
  list_empty_font: Option<String>,
}
pub struct Config {
  pub window_width_percent: u32,
  pub window_height_percent: u32,
  pub entry_height: u32,
  pub list_item_height: u32,
  pub entry_font: String,
  pub list_font: String,
  pub list_empty_font: String,
}

impl Config {
  pub fn load () -> Self {
    let pathname = "/home/j/.config/launcher.toml";
    let parsed = if let Ok (content) = std::fs::read_to_string (pathname) {
      toml::from_str (&content).unwrap_or_else (|error| {
        eprintln! ("Config loading error: {error}");
        ParsedConfig::default ()
      })
    } else {
      ParsedConfig::default ()
    };
    Config {
      window_width_percent: parsed.window_width_percent.unwrap_or (50),
      window_height_percent: parsed.window_height_percent.unwrap_or (50),
      entry_height: parsed.entry_height.unwrap_or (48),
      list_item_height: parsed.list_item_height.unwrap_or (44),
      entry_font: parsed.entry_font.unwrap_or_else (|| "sans 24".to_string ()),
      list_font: parsed.list_font.unwrap_or_else (|| "sans 20".to_string ()),
      list_empty_font: parsed
        .list_empty_font
        .unwrap_or_else (|| "sans 48".to_string ()),
    }
  }
}
