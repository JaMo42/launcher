use crate::config::ICON_THEME;
use cairo::Pattern;
use gio::{Cancellable, File, MemoryInputStream};
use glib::Bytes;
use rsvg::{CairoRenderer, Loader, SvgHandle};

pub mod resources {
    pub static SEARCH_ICON: &[u8] = include_bytes!("../res/search.svg");
    pub static HISTORY_ICON: &[u8] = include_bytes!("../res/history.svg");
    pub static LANGUAGE_ICON: &[u8] = include_bytes!("../res/language.svg");
    pub static FOLDER_OPEN_ICON: &[u8] = include_bytes!("../res/folder_open.svg");
    pub static TERMINAL_ICON: &[u8] = include_bytes!("../res/terminal.svg");
    pub static CALCULATE_ICON: &[u8] = include_bytes!("../res/calculate.svg");
    pub static CONVERSION_PATH_ICON: &[u8] = include_bytes!("../res/conversion_path.svg");
    pub static WARNING_ICON: &[u8] = include_bytes!("../res/warning.svg");
}

pub struct Svg {
    pub renderer: CairoRenderer<'static>,
    _handle: Box<SvgHandle>,
    pub pattern: Option<Pattern>,
}

impl Svg {
    pub fn load(data: &'static [u8]) -> Self {
        let bytes = Bytes::from_static(data);
        let stream = MemoryInputStream::from_bytes(&bytes);
        let handle = Box::new(
            Loader::new()
                .read_stream(&stream, None::<&File>, None::<&Cancellable>)
                .unwrap(),
        );
        // We can just give the renderer a static reference since the lifetime of
        // the renderer and the handle are both tied to the `Svg` object.
        let static_handle: &'static _ = unsafe { &*(handle.as_ref() as *const SvgHandle) };
        let renderer = CairoRenderer::new(static_handle);
        Self {
            renderer,
            _handle: handle,
            pattern: None,
        }
    }

    pub fn open(path: &str) -> Self {
        let handle = Box::new(Loader::new().read_path(path).unwrap());
        let static_handle: &'static _ = unsafe { &*(handle.as_ref() as *const SvgHandle) };
        let renderer = CairoRenderer::new(static_handle);
        Self {
            renderer,
            _handle: handle,
            pattern: None,
        }
    }
}

pub fn find_icon(name: &str) -> Option<String> {
    ICON_THEME.with_borrow(|t| t.lookup(name))
}
