use cairo::Pattern;
use gio::{Cancellable, File, MemoryInputStream};
use glib::Bytes;
use librsvg::{CairoRenderer, Loader, SvgHandle};

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
