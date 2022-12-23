use crate::{
  app::{send_signal, Signal},
  cache::DesktopEntryCache,
  draw::DrawingContext,
  entry::Entry,
  input::KeyEvent,
  layout::{Layout, Rectangle},
  list_view::ListView,
  x::{display::ScopedInputGrab, Display, Window},
};
use std::{
  ffi::c_void,
  sync::{mpsc::Sender, Arc, Mutex},
};
use x11::xlib::{AllocNone, Button4, Button5, ButtonPressMask, TrueColor, XButtonPressedEvent};

pub mod colors {
  use crate::draw::Color;

  pub const BACKGROUND: Color = Color::new (44, 44, 46, 204);
  pub const TEXT: Color = Color::new (174, 174, 178, 255);
  pub const ACCENT: Color = Color::new (10, 132, 255, 255);

  pub const ENTRY_BACKGROUND: Color = BACKGROUND.scale (90);
  pub const ENTRY_CURSOR: Color = TEXT.scale (125);
  pub const ENTRY_SELECTION: Color = ENTRY_FOCUSED_BORDER.with_alpha (96);
  pub const ENTRY_FOCUSED_BORDER: Color = ACCENT;
  pub const ENTRY_NORMAL_BORDER: Color = BACKGROUND.scale (110);
  pub const ENTRY_PLACEHOLDER_TEXT: Color = TEXT.scale (70);

  pub const LIST_MATCH_NAME: Color = TEXT.scale (70);
  pub const LIST_LIGHT_BACKGROUND: Color = BACKGROUND.scale (120);
  pub const LIST_MATCH_HIGHLIGHT: Color = ACCENT;
  pub const LIST_SELECTED_BACKGROUND: Color = BACKGROUND.scale (60);
}

fn main_screen_size (display: &Display) -> (u32, u32) {
  use x11::xinerama::*;
  use x11::xlib::XFree;
  unsafe {
    if XineramaIsActive (display.as_raw ()) == 0 {
      display.size ()
    } else {
      let mut len = 0;
      let data = XineramaQueryScreens (display.as_raw (), &mut len);
      let result = std::slice::from_raw_parts (data, len as usize).to_vec ();
      XFree (data as *mut c_void);
      for screen_info in &result {
        if screen_info.screen_number == 0 {
          return (screen_info.width as u32, screen_info.height as u32);
        }
      }
      (result[0].width as u32, result[0].height as u32)
    }
  }
}

pub struct UI {
  display: Display,
  pub main_window: Window,
  entry: Entry,
  pub list_view: ListView,
  input_focus: bool,
  width: i32,
  height: i32,
  valid_click_rect: Rectangle,
  signal_sender: Sender<Signal>,
  _input_grab: ScopedInputGrab,
}

impl UI {
  pub fn new (
    display: &Display,
    signal_sender: Sender<Signal>,
    cache: Arc<Mutex<DesktopEntryCache>>,
  ) -> Self {
    let screen_size = main_screen_size (&display);
    let layout = Layout::new (screen_size.0, screen_size.1, 1.0);
    let width = layout.window.width;
    let height = layout.window.height;
    let visual_info = display.match_visual_info (32, TrueColor).unwrap ();
    let colormap = display.create_colormap (visual_info.visual, AllocNone);
    let main_window = Window::builder (&display)
      .size (width, height)
      .position (
        (screen_size.0 - width) as i32 / 2,
        (screen_size.1 - height) as i32 / 2,
      )
      .attributes (|attributes| {
        attributes
          .background_pixel (colors::BACKGROUND.pack ())
          //.override_redirect (true)
          .colormap (colormap)
          .border_pixel (0);
      })
      .visual (visual_info.visual)
      .depth (visual_info.depth)
      .build ();
    main_window.set_class_hint ("Launcher", "launcher");
    main_window.map_raised ();
    main_window.clear ();

    let mut dc = DrawingContext::create (display, width, height, &visual_info);
    dc.fill (colors::BACKGROUND);
    dc.render (main_window, &Rectangle::new (0, 0, width, height));
    dc.destroy ();

    let p = layout.entry.reparent;
    let entry = Entry::create (
      display,
      signal_sender.clone (),
      layout.entry,
      &visual_info,
      colormap,
    );
    entry.window.reparent (main_window, p.0, p.1);

    let p = layout.list_view.reparent;
    let valid_click_rect = Rectangle::new (
      p.0,
      p.1,
      layout.list_view.window.width,
      layout.list_view.window.height,
    );
    let list_view = ListView::create (
      display,
      signal_sender.clone (),
      layout.list_view,
      &visual_info,
      colormap,
      cache,
    );
    list_view.window.reparent (main_window, p.0, p.1);

    main_window.map_subwindows ();

    Self {
      display: *display,
      main_window,
      entry,
      list_view,
      input_focus: true,
      width: width as i32,
      height: height as i32,
      valid_click_rect,
      signal_sender,
      _input_grab: display.scoped_input_grab (main_window, ButtonPressMask),
    }
  }

  pub fn redraw (&mut self) {
    self.entry.draw ();
    self.entry.draw_cursor_and_selection ();
    self.list_view.draw ();
  }

  pub fn text_input (&mut self, text: &str) {
    if self.input_focus {
      self.entry.text_input (text);
    }
  }

  pub fn key_press (&mut self, event: KeyEvent) {
    if self.input_focus {
      self.entry.key_press (event);
    } else {
      self.list_view.key_press (event);
    }
  }

  pub fn button_press (&mut self, event: &mut XButtonPressedEvent) {
    // Button4 and Button5 are the mouse whell, we can always allow it.
    if event.button != Button4 && event.button != Button5 {
      if event.x < 0 || event.y < 0 || event.x > self.width || event.y > self.height {
        // Not inside the main window, close the program.
        send_signal (&self.display, &self.signal_sender, Signal::Quit);
        return;
      }
      if event.x < self.valid_click_rect.x
        || event.x >= self.valid_click_rect.x + self.valid_click_rect.width as i32
        || event.y < self.valid_click_rect.y
        || event.y >= self.valid_click_rect.y + self.valid_click_rect.height as i32
      {
        // Not inside the list window, we don't care about it.
        return;
      }
    }
    // Translate from main window to list window.
    event.x -= self.valid_click_rect.x;
    event.y -= self.valid_click_rect.y;
    self.list_view.button_press (event);
  }

  pub fn swap_focus (&mut self) {
    self.input_focus = !self.input_focus;
    if !self.input_focus && self.list_view.is_empty () {
      self.input_focus = true;
    } else {
      self.entry.set_focused (self.input_focus);
    }
  }
}

impl Drop for UI {
  fn drop (&mut self) {
    self.main_window.destroy ();
  }
}
