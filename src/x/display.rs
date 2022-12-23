use super::{window::ToXWindow, *};
use std::ffi::CString;

#[derive(Copy, Clone)]
pub struct Display {
  connection: XDisplay,
  screen: c_int,
  root: XWindow,
}

impl Display {
  pub fn connect (name: Option<&str>) -> Self {
    let connection;
    let root;
    let screen;

    unsafe {
      connection = XOpenDisplay (
        name
          .map (|s| s.as_ptr () as *const c_char)
          .unwrap_or (std::ptr::null ()),
      );
      if connection.is_null () {
        let name = name
          .map (|s| s.to_string ())
          .or_else (|| std::env::var ("DISPLAY").ok ())
          .unwrap_or_default ();
        panic! ("Could not open display: {}", name);
      }
      root = XDefaultRootWindow (connection);
      screen = XDefaultScreen (connection);
    }

    Self {
      connection,
      screen,
      root,
    }
  }

  pub fn root (&self) -> XWindow {
    self.root
  }

  pub fn width (&self) -> u32 {
    unsafe { XDisplayWidth (self.connection, self.screen) as u32 }
  }

  pub fn height (&self) -> u32 {
    unsafe { XDisplayHeight (self.connection, self.screen) as u32 }
  }

  pub fn size (&self) -> (u32, u32) {
    (self.width (), self.height ())
  }

  pub fn close (&mut self) {
    if !self.connection.is_null () {
      unsafe {
        XCloseDisplay (self.connection);
      }
      self.connection = std::ptr::null_mut ();
    }
  }

  pub fn as_raw (&self) -> XDisplay {
    self.connection
  }

  pub fn flush (&self) {
    unsafe {
      XFlush (self.connection);
    }
  }

  pub fn sync (&self, discard_events: bool) {
    unsafe {
      XSync (self.connection, discard_events as i32);
    }
  }

  pub fn next_event (&self, event_out: &mut XEvent) {
    unsafe {
      XNextEvent (self.connection, event_out);
    }
  }

  pub fn mask_event (&self, mask: i64, event_out: &mut XEvent) {
    unsafe {
      XMaskEvent (self.connection, mask, event_out);
    }
  }

  pub fn push_event (&self, mut event: XEvent) {
    unsafe {
      XPutBackEvent (self.connection, &mut event);
    }
  }

  pub fn set_input_focus<W: ToXWindow> (&self, window: W) {
    unsafe {
      XSetInputFocus (
        self.connection,
        window.to_xwindow (),
        RevertToParent,
        CurrentTime,
      );
    }
  }

  pub fn intern_atom (&self, name: &str) -> Atom {
    unsafe {
      let cstr = CString::new (name).unwrap ();
      XInternAtom (self.connection, cstr.as_ptr (), FALSE)
    }
  }

  pub fn get_atom_name (&self, atom: Atom) -> String {
    unsafe {
      CStr::from_ptr (XGetAtomName (self.connection, atom))
        .to_str ()
        .unwrap ()
        .to_owned ()
    }
  }

  pub fn get_selection_by_name (&self, name: &str) -> Window {
    let selection = self.intern_atom (name);
    Window::from_handle (self, unsafe {
      XGetSelectionOwner (self.connection, selection)
    })
  }

  pub fn match_visual_info (&self, depth: i32, class: i32) -> Option<XVisualInfo> {
    unsafe {
      let mut vi: XVisualInfo = std::mem::MaybeUninit::zeroed ().assume_init ();
      if XMatchVisualInfo (self.connection, self.screen, depth, class, &mut vi) != 0 {
        Some (vi)
      } else {
        None
      }
    }
  }

  pub fn create_colormap (&self, visual: *mut Visual, alloc: i32) -> Colormap {
    unsafe { XCreateColormap (self.connection, self.root, visual, alloc) }
  }

  pub fn grab_pointer (&self, window: Window, mask: i64) -> bool {
    unsafe {
      XGrabPointer (
        self.connection,
        window.to_xwindow (),
        FALSE,
        mask as u32,
        GrabModeAsync,
        GrabModeAsync,
        NONE,
        NONE,
        CurrentTime,
      ) == GrabSuccess
    }
  }

  pub fn scoped_pointer_grab (&self, window: Window, mask: i64) -> Option<ScopedPointerGrab> {
    if self.grab_pointer (window, mask) {
      Some (ScopedPointerGrab {
        display: self.connection,
      })
    } else {
      None
    }
  }

  pub fn scoped_keyboard_grab (&self, window: Window) -> ScopedKeyboardGrab {
    unsafe {
      XGrabKeyboard (
        self.connection,
        window.handle (),
        False,
        GrabModeAsync,
        GrabModeAsync,
        CurrentTime,
      );
    }
    ScopedKeyboardGrab {
      connection: self.connection,
    }
  }

  pub fn scoped_input_grab (
    &self,
    window: Window,
    mouse_mask: i64,
  ) -> (ScopedKeyboardGrab, Option<ScopedPointerGrab>) {
    (
      self.scoped_keyboard_grab (window),
      self.scoped_pointer_grab (window, mouse_mask),
    )
  }
}

pub trait ToXDisplay {
  fn to_xdisplay (&self) -> XDisplay;
}

impl ToXDisplay for Display {
  fn to_xdisplay (&self) -> XDisplay {
    self.as_raw ()
  }
}

impl ToXDisplay for XDisplay {
  fn to_xdisplay (&self) -> XDisplay {
    *self
  }
}

pub struct ScopedPointerGrab {
  display: XDisplay,
}

impl Drop for ScopedPointerGrab {
  fn drop (&mut self) {
    unsafe {
      XUngrabPointer (self.display, CurrentTime);
    }
  }
}

pub struct ScopedKeyboardGrab {
  connection: XDisplay,
}

impl Drop for ScopedKeyboardGrab {
  fn drop (&mut self) {
    unsafe {
      XUngrabKeyboard (self.connection, CurrentTime);
    }
  }
}

pub type ScopedInputGrab = (ScopedKeyboardGrab, Option<ScopedPointerGrab>);
