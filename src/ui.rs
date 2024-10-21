use crate::{
    app::{send_signal, Signal},
    cache::DesktopEntryCache,
    config::Config,
    draw::DrawingContext,
    entry::Entry,
    input::KeyEvent,
    layout::{Layout, Rectangle},
    list_view::{ListView, Render},
    smart_content::{ReadyContent, SmartContent},
    x::{display::ScopedInputGrab, Display, Window},
};
use std::{
    ffi::c_void,
    sync::{mpsc::Sender, Arc, Mutex},
};
use x11::xlib::{
    AllocNone, Button4, Button5, ButtonPressMask, KeyPressMask, TrueColor, XButtonPressedEvent,
};

pub mod colors {
    use crate::draw::Color;

    pub const BACKGROUND: Color = Color::new(44, 44, 46, 204);
    pub const TEXT: Color = Color::new(174, 174, 178, 255);
    pub const ACCENT: Color = Color::new(10, 132, 255, 255);

    pub const ENTRY_BACKGROUND: Color = BACKGROUND.scale(90);
    pub const ENTRY_CURSOR: Color = TEXT.scale(125);
    pub const ENTRY_SELECTION: Color = ENTRY_FOCUSED_BORDER.with_alpha(96);
    pub const ENTRY_FOCUSED_BORDER: Color = ACCENT;
    pub const ENTRY_NORMAL_BORDER: Color = BACKGROUND.scale(110);
    pub const ENTRY_PLACEHOLDER_TEXT: Color = TEXT.scale(70);

    pub const LIST_MATCH_NAME: Color = TEXT.scale(70);
    pub const LIST_LIGHT_BACKGROUND: Color = BACKGROUND.scale(120);
    pub const LIST_MATCH_HIGHLIGHT: Color = ACCENT;
    pub const LIST_SELECTED_BACKGROUND: Color = BACKGROUND.scale(60).with_alpha(229);
    pub const LIST_SCROLL_BAR: Color = TEXT.with_alpha(204).scale(50);
}

fn main_screen_size(display: &Display) -> (u32, u32) {
    use x11::xinerama::*;
    use x11::xlib::XFree;
    unsafe {
        if XineramaIsActive(display.as_raw()) == 0 {
            display.size()
        } else {
            let mut len = 0;
            let data = XineramaQueryScreens(display.as_raw(), &mut len);
            let result = std::slice::from_raw_parts(data, len as usize).to_vec();
            XFree(data as *mut c_void);
            for screen_info in &result {
                if screen_info.screen_number == 0 {
                    return (screen_info.width as u32, screen_info.height as u32);
                }
            }
            (result[0].width as u32, result[0].height as u32)
        }
    }
}

pub struct Ui {
    display: Display,
    pub main_window: Window,
    entry: Entry,
    // The list view was designed for a variable layout, and just adding a
    // second one is quite painless.
    full_list_view: ListView,
    reduced_list_view: ListView,
    pub smart_content: SmartContent,
    showing_smart_content: bool,
    input_focus: bool,
    width: i32,
    height: i32,
    signal_sender: Sender<Signal>,
    _input_grab: ScopedInputGrab,
}

impl Ui {
    pub fn new(
        display: &Display,
        signal_sender: Sender<Signal>,
        cache: Arc<Mutex<DesktopEntryCache>>,
        config: &Config,
    ) -> Self {
        let screen_size = main_screen_size(display);
        let visual_info = display.match_visual_info(32, TrueColor).unwrap();
        let colormap = display.create_colormap(visual_info.visual, AllocNone);

        let window_size = Layout::window_size(screen_size.0, screen_size.1, config);
        let mut dc = DrawingContext::create(display, window_size.0, window_size.1, &visual_info);

        let layout = Layout::new(screen_size.0, screen_size.1, config, |font| {
            let layout = dc.layout();
            layout.set_font_description(Some(font));
            layout.set_text("Mgj가|^");
            layout.size().1 / pango::SCALE
        });
        let width = layout.window.width;
        let height = layout.window.height;

        let main_window = Window::builder(display)
            .size(width, height)
            .position(
                (screen_size.0 - width) as i32 / 2,
                (screen_size.1 - height) as i32 / 2,
            )
            .attributes(|attributes| {
                attributes
                    .background_pixel(colors::BACKGROUND.pack())
                    .override_redirect(!cfg!(debug_assertions))
                    .colormap(colormap)
                    .border_pixel(0)
                    .event_mask(KeyPressMask | ButtonPressMask);
            })
            .visual(visual_info.visual)
            .depth(visual_info.depth)
            .build();
        main_window.set_class_hint("Launcher", "launcher");

        let p = layout.entry.reparent;
        let entry = Entry::create(
            display,
            signal_sender.clone(),
            layout.entry,
            &visual_info,
            colormap,
            config,
        );
        entry.window.reparent(main_window, p.0, p.1);

        let p = layout.smart_content.reparent;
        let smart_content = SmartContent::create(
            display,
            layout.smart_content,
            &visual_info,
            colormap,
            config,
        );
        smart_content.window.reparent(main_window, p.0, p.1);

        let p = layout.full_list_view.reparent;
        let full_list_view = ListView::create(
            display,
            signal_sender.clone(),
            layout.full_list_view,
            &visual_info,
            colormap,
            cache.clone(),
            config,
        );
        full_list_view.window.reparent(main_window, p.0, p.1);

        let p = layout.reduced_list_view.reparent;
        let reduced_list_view = ListView::create(
            display,
            signal_sender.clone(),
            layout.reduced_list_view,
            &visual_info,
            colormap,
            cache,
            config,
        );
        reduced_list_view.window.reparent(main_window, p.0, p.1);

        // Map all windows and draw background
        main_window.map_subwindows();
        // Smart content is only visibe when there is something to show, and
        // since we create the list view with its full size it would overlap.
        smart_content.window.unmap();
        reduced_list_view.window.unmap();
        dc.fill(colors::BACKGROUND);
        main_window.map_raised();
        dc.render(main_window, &Rectangle::new(0, 0, width, height));
        dc.destroy();
        display.set_input_focus(main_window);

        Self {
            display: *display,
            main_window,
            entry,
            full_list_view,
            reduced_list_view,
            smart_content,
            showing_smart_content: false,
            input_focus: true,
            width: width as i32,
            height: height as i32,
            signal_sender,
            _input_grab: display.scoped_input_grab(main_window, ButtonPressMask),
        }
    }

    fn layout(&mut self, show_smart_content: bool) {
        if show_smart_content {
            self.smart_content.window.map_raised();
            self.reduced_list_view.window.map_raised();
            self.full_list_view.window.unmap();
        } else {
            self.smart_content.window.unmap();
            self.reduced_list_view.window.unmap();
            self.full_list_view.window.map_raised();
        }
        self.showing_smart_content = show_smart_content;
    }

    pub fn list_view(&mut self) -> &mut ListView {
        if self.showing_smart_content {
            &mut self.reduced_list_view
        } else {
            &mut self.full_list_view
        }
    }

    pub fn redraw(&mut self) {
        self.entry.draw();
        self.entry.draw_cursor_and_selection();
        self.list_view().draw();
        if self.showing_smart_content {
            self.smart_content.draw();
        }
    }

    pub fn text_input(&mut self, text: &str) {
        if self.input_focus {
            self.entry.text_input(text);
        }
    }

    pub fn set_items<T: Render + 'static>(&mut self, items: &[T], search: &str) {
        self.full_list_view
            .set_items(items, search, self.showing_smart_content);
        self.reduced_list_view
            .set_items(items, search, !self.showing_smart_content);
    }

    pub fn set_smart_content(&mut self, content: Option<ReadyContent>) {
        if let Some(text) = content {
            self.smart_content.set(text);
            self.layout(true);
            self.smart_content.draw();
        } else if self.showing_smart_content {
            self.smart_content.window.unmap();
            self.layout(false);
        }
    }

    pub fn showing_useful_smart_content(&self) -> bool {
        self.showing_smart_content && self.smart_content.is_useful()
    }

    pub fn key_press(&mut self, event: KeyEvent) {
        if self.input_focus {
            self.entry.key_press(event);
        } else {
            self.list_view().key_press(event);
        }
    }

    pub fn button_press(&mut self, event: &mut XButtonPressedEvent) {
        // Button4 and Button5 are the mouse wheel, we can always allow it.
        if event.button != Button4 && event.button != Button5 {
            if event.x < 0 || event.y < 0 || event.x > self.width || event.y > self.height {
                // Not inside the main window, close the program.
                send_signal(&self.display, &self.signal_sender, Signal::Quit);
                return;
            }
        }
        if self.entry.hit_test(event.x, event.y) {
            self.entry.set_focused(true);
            self.input_focus = true;
            self.smart_content.set_selected(false);
        } else if self.showing_smart_content && self.smart_content.hit_test(event.x, event.y) {
            self.entry.set_focused(false);
            self.input_focus = false;
            self.smart_content.set_selected(true);
        } else if self.list_view().hit_test(event.x, event.y) {
            self.entry.set_focused(false);
            self.input_focus = false;
            self.list_view().button_press(event);
            self.smart_content.set_selected(false);
        }
    }

    pub fn swap_focus(&mut self) {
        self.input_focus = !self.input_focus;
        if !self.input_focus && self.list_view().is_empty() {
            self.input_focus = true;
        } else {
            self.entry.set_focused(self.input_focus);
        }
    }
}

impl Drop for Ui {
    fn drop(&mut self) {
        self.main_window.unmap();
        self.main_window.destroy();
        self.display.sync(true);
        self.display.flush();
    }
}
