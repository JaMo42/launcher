use cairo::Pattern;
use gio::{Cancellable, File, MemoryInputStream};
use glib::Bytes;
use librsvg::{CairoRenderer, Loader, SvgHandle};

use crate::config::icon_search_path;

pub mod resources {
  pub static SEARCH_ICON: &'static [u8] = include_bytes! ("../res/search.svg");
}

pub struct Svg {
  pub renderer: CairoRenderer<'static>,
  _handle: Box<SvgHandle>,
  pub pattern: Option<Pattern>,
}

impl Svg {
  pub fn load (data: &'static [u8]) -> Self {
    let bytes = Bytes::from_static (data);
    let stream = MemoryInputStream::from_bytes (&bytes);
    let handle = Box::new (
      Loader::new ()
        .read_stream (&stream, None::<&File>, None::<&Cancellable>)
        .unwrap (),
    );
    // We can just give the renderer a static reference since the lifetime of
    // the renderer and the handle are both tied to the `Svg` object.
    let static_handle: &'static _ = unsafe { &*(handle.as_ref () as *const SvgHandle) };
    let renderer = CairoRenderer::new (static_handle);
    Self {
      renderer,
      _handle: handle,
      pattern: None,
    }
  }

  pub fn open (path: &str) -> Self {
    let handle = Box::new (Loader::new ().read_path (path).unwrap ());
    let static_handle: &'static _ = unsafe { &*(handle.as_ref () as *const SvgHandle) };
    let renderer = CairoRenderer::new (static_handle);
    Self {
      renderer,
      _handle: handle,
      pattern: None,
    }
  }
}

pub fn find_icon (name: &str) -> Option<String> {
  let base = icon_search_path ();
  if base.is_empty () {
    return None;
  }
  let dirs = [
    "apps",
    "places",
    "devices",
    "actions",
    "categories",
    "mimetypes",
    "status",
    "emotes",
    "intl",
    "emblems",
  ];
  for d in dirs {
    let pathname = format! ("{}/48x48/{}/{}.svg", base, d, name);
    if std::fs::metadata (&pathname).is_ok () {
      return Some (pathname);
    }
  }
  None
}
