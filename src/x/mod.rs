// Copied and modified from https://github.com/JaMo42/window_manager
use std::ffi::*;
use x11::xlib::*;

pub type XDisplay = *mut x11::xlib::Display;
pub type XWindow = x11::xlib::Window;

pub const NONE: c_ulong = 0;
pub const FALSE: c_int = 0;
#[allow(dead_code)]
pub const TRUE: c_int = 1;

pub mod display;
pub mod window;
pub mod window_builder;

// Shadow xlib types with wrappers
pub use display::Display;
pub use window::Window;

pub fn lookup_keysym(event: &XKeyEvent) -> KeySym {
    unsafe { XLookupKeysym(event as *const XKeyEvent as *mut XKeyEvent, 0) }
}

pub fn init_threads() -> Status {
    unsafe { XInitThreads() }
}
