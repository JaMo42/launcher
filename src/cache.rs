use crate::{
  res::find_icon,
  search::{MatchKind, SIMILARITY_THRESHHOLD},
};
use freedesktop_desktop_entry::DesktopEntry;

/// Get the `lang`, `COUNTRY`, and `MODIFIER` parts from `LC_MESSAGES` or `LANG`.
fn get_locale () -> Option<(String, Option<String>, Option<String>)> {
  let mut locale = std::env::var ("LC_MESSAGES")
    .or_else (|_| std::env::var ("LANG"))
    .ok ()?;
  let mut country = None;
  let mut modifier = None;
  if let Some (modifier_tag) = locale.chars ().position (|c| c == '@') {
    modifier = Some (locale[(modifier_tag + 1)..].to_string ());
    locale.replace_range (modifier_tag.., "");
  }
  if let Some (encoding) = locale.chars ().position (|c| c == '.') {
    locale.replace_range (encoding.., "");
  }
  if let Some (country_tag) = locale.chars ().position (|c| c == '_') {
    country = Some (locale[(country_tag + 1)..].to_string ());
    locale.replace_range (country_tag.., "");
  }
  Some ((locale, country, modifier))
}

#[derive(Copy, Clone)]
pub enum MatchField {
  Name (MatchKind),
  LocalizedName (MatchKind),
  GenericName (MatchKind),
  LocalizedGenericName (MatchKind),
  FileName (MatchKind),
}

impl MatchField {
  pub fn inner (&self) -> MatchKind {
    match self {
      &Self::Name (kind) => kind,
      &Self::LocalizedName (kind) => kind,
      &Self::GenericName (kind) => kind,
      &Self::LocalizedGenericName (kind) => kind,
      &Self::FileName (kind) => kind,
    }
  }
}

pub struct Match {
  pub id: usize,
  pub field: MatchField,
}

#[derive(Clone)]
pub struct Entry {
  pub name: String,
  pub localized_name: Option<String>,
  pub generic_name: Option<String>,
  pub localized_generic_name: Option<String>,
  pub file_name: String,
  pub exec: String,
  pub icon: Option<String>,
}

impl Entry {
  pub fn from_desktop_entry (
    file_name: String,
    de: &DesktopEntry,
    locales: &[String],
  ) -> Option<Self> {
    let mut localized_name = None;
    let mut localized_generic_name = None;
    for locale in locales {
      if let Some (n) = de.name (Some (locale)) {
        localized_name = Some (n.to_string ());
        break;
      }
    }
    for locale in locales {
      if let Some (n) = de.generic_name (Some (locale)) {
        localized_generic_name = Some (n.to_string ());
        break;
      }
    }
    let generic_name = de
      .generic_name (None)
      .map (|cow_str| cow_str.to_string ())
      .or_else (|| localized_generic_name.clone ());
    let name = de
      .name (None)
      .map (|cow_str| cow_str.to_string ())
      .or_else (|| localized_name.clone ())
      .or_else (|| generic_name.clone ());
    if name.is_none () {
      eprintln! ("No suitable name found in {}.", file_name);
      None
    } else {
      Some (Self {
        name: name.unwrap (),
        localized_name,
        generic_name,
        localized_generic_name,
        file_name,
        // Already checked this exists in `DesktopEntryCache::rebuild`.
        exec: de.exec ().unwrap ().to_string (),
        icon: de.icon ().and_then (|s| find_icon (s)),
      })
    }
  }

  pub fn get_field (&self, field: MatchField) -> &str {
    match field {
      MatchField::Name (_) => &self.name,
      MatchField::LocalizedName (_) => self.localized_name.as_ref ().unwrap (),
      MatchField::GenericName (_) => self.generic_name.as_ref ().unwrap (),
      MatchField::LocalizedGenericName (_) => self.localized_generic_name.as_ref ().unwrap (),
      MatchField::FileName (_) => &self.file_name,
    }
  }
}

pub struct DesktopEntryCache {
  entries: Vec<Entry>,
  locale: Option<String>,
  error: Option<std::io::Error>,
}

impl DesktopEntryCache {
  pub fn new (locale: &Option<String>) -> Self {
    Self {
      entries: Vec::with_capacity (128),
      locale: locale.clone (),
      error: None,
    }
  }

  /// Get a list of locales to try to get the localized names for.
  ///
  /// If the user specified a locale name, only that is used no matter what it is.
  ///
  /// Other wise the values are derived from the `LC_MESSAGES` locale, or if that's
  /// not set the `LANG` locale.
  ///
  /// The locale is split into `lang`, `COUNTRY`, and `MODIFIER` from this
  /// pattern: `lang_COUNTRY.ENCODING@MODIFIER`.
  /// Then the following combinations are tried:
  /// ```text
  /// lang_COUNTRY@MODIFIER
  /// lang_COUNTRY
  /// lang@MODIFIER
  /// lang
  /// ```
  fn get_locales (&self) -> Vec<String> {
    if let Some (configured) = &self.locale {
      vec! [configured.to_owned ()]
    } else if let Some ((lang, country, modifier)) = get_locale () {
      // https://specifications.freedesktop.org/desktop-entry-spec/desktop-entry-spec-latest.html#localized-keys
      let mut locales = Vec::with_capacity (4);
      if let Some (country) = &country {
        if let Some (modifier) = &modifier {
          locales.push (format! ("{}_{}@{}", lang, country, modifier));
        }
        locales.push (format! ("{}_{}", lang, country));
      }
      if let Some (modifier) = &modifier {
        locales.push (format! ("{}@{}", lang, modifier));
      }
      locales.push (lang);
      locales
    } else {
      Vec::new ()
    }
  }

  pub fn rebuild (&mut self) {
    if !self.entries.is_empty () {
      self.entries.clear ();
    }
    let locales = self.get_locales ();
    let dir = std::fs::read_dir ("/usr/share/applications");
    if let Err (error) = dir {
      eprintln! ("Could not read /usr/share/applications: {error}");
      self.error = Some (error);
      return;
    }
    self.error = None;
    for file in dir.unwrap ().flatten () {
      let file_name = if let Some (file_name) = file.file_name ().to_str () {
        file_name.to_owned ()
      } else {
        continue;
      };
      if !file_name.ends_with (".desktop") {
        continue;
      }
      let content = std::fs::read_to_string (file.path ());
      if let Err (error) = content {
        eprintln! ("Could not read {}: {}", file_name, error);
        continue;
      }
      let path = file.path ().as_path ().to_owned ();
      let maybe_de = DesktopEntry::decode (&path, content.as_ref ().unwrap ());
      if let Err (error) = maybe_de {
        eprintln! ("Could not decode {}: {}", file_name, error);
        continue;
      }
      let de = maybe_de.unwrap ();
      if de.exec ().is_none () {
        continue;
      }
      if let Some (entry) = Entry::from_desktop_entry (file_name, &de, &locales) {
        self.entries.push (entry);
      }
    }
  }

  fn get_match (name: &str, entry_value: String) -> Option<MatchKind> {
    if entry_value == name {
      Some (MatchKind::Exact)
    } else {
      let sim = strsim::jaro_winkler (&name, &entry_value);
      if sim >= SIMILARITY_THRESHHOLD {
        Some (MatchKind::Similar (sim))
      } else {
        None
      }
    }
  }

  pub fn find_all (&self, name: &str) -> Vec<Match> {
    self.find_subset (name, 0..self.entries.len ())
  }

  pub fn find_subset<T> (&self, name: &str, set: T) -> Vec<Match>
  where
    T: IntoIterator<Item = usize>,
  {
    let mut matches = Vec::new ();
    for id in set.into_iter () {
      let entry = &self.entries[id];
      macro_rules! check {
        ($field:expr, $match_field:ident) => {
          if let Some (value) = $field {
            if let Some (match_) = Self::get_match (&name, value.to_lowercase ()) {
              matches.push (Match {
                id,
                field: MatchField::$match_field (match_),
              });
              continue;
            }
          }
        };
      }
      // TODO: a value with lower prioty could still get a higher score, to
      //       accommodate for this we should chose the maximum score of these
      //       instead of shortcircuting on the first match.
      check! (entry.localized_name.as_ref (), LocalizedName);
      check! (Some (&entry.name), Name);
      check! (entry.localized_generic_name.as_ref (), LocalizedGenericName);
      check! (entry.generic_name.as_ref (), GenericName);
      check! (Some (&entry.file_name), FileName);
    }
    matches
  }

  pub fn find_file (&self, file_name: &str) -> Option<usize> {
    for (id, entry) in self.entries.iter ().enumerate () {
      if entry.file_name == file_name {
        return Some (id);
      }
    }
    None
  }

  pub fn error (&self) -> Option<&std::io::Error> {
    self.error.as_ref ()
  }

  pub fn get_entry (&self, id: usize) -> &Entry {
    &self.entries[id]
  }
}
