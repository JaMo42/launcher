use crate::{
    app::{send_signal, Signal},
    cache::DesktopEntryCache,
    config::Config,
    draw::DrawingContext,
    input::{Key, KeyEvent},
    layout::{ListViewLayout, Rectangle},
    res::{resources, Svg},
    ui::colors,
    x::{Display, Window},
};
use pango::{EllipsizeMode, FontDescription};
use std::{
    ops::Deref,
    sync::{mpsc::Sender, Arc, Mutex},
};
use x11::xlib::{Button1, Button4, Button5, Colormap, XButtonPressedEvent, XVisualInfo};

const CAPACITY: u32 = 100;

pub struct Item {
    icon: Option<Svg>,
    markup_text: String,
    is_in_history: bool,
}

pub trait Render {
    fn icon(&self, _cache: &DesktopEntryCache) -> Option<Svg> {
        None
    }

    fn is_in_history(&self) -> bool {
        false
    }

    fn markup(&self, search: &str, cache: &DesktopEntryCache) -> String;
}

enum LazyItem {
    Rendered(Item),
    NotRendered(&'static dyn Render),
}

impl LazyItem {
    fn new(renderable: &'static dyn Render) -> Self {
        Self::NotRendered(renderable)
    }

    fn get(&mut self, search: &str, cache: &Arc<Mutex<DesktopEntryCache>>) -> &Item {
        match *self {
            Self::Rendered(ref item) => item,
            Self::NotRendered(renderable) => {
                {
                    let guard = cache.lock().unwrap();
                    let cache = guard.deref();
                    *self = Self::Rendered(Item {
                        icon: renderable.icon(cache),
                        markup_text: renderable.markup(search, cache),
                        is_in_history: renderable.is_in_history(),
                    });
                }
                self.get(search, cache)
            }
        }
    }

    fn is_rendered(&self) -> bool {
        matches!(self, &Self::Rendered(_))
    }
}

fn create_empty_screen(
    display: &Display,
    width: u32,
    height: u32,
    visual_info: &XVisualInfo,
    font: &str,
) -> DrawingContext {
    let mut empty_screen = DrawingContext::create(display, width, height, visual_info);
    empty_screen.fill(colors::BACKGROUND);
    empty_screen.set_color(colors::TEXT);
    empty_screen.set_font(&FontDescription::from_string(font));
    empty_screen
        .text("No results", Rectangle::new(0, 0, width, height), false)
        .center_width()
        .center_height()
        .draw();
    empty_screen
}

pub struct ListView {
    pub window: Window,
    pub display: Display,
    signal_sender: Sender<Signal>,
    layout: ListViewLayout,
    dc: DrawingContext,
    items: Vec<LazyItem>,
    scroll: i32,
    max_scroll_offset: i32,
    selected: usize,
    click_item: usize,
    click_time: u64,
    search: String,
    empty_screen: DrawingContext,
    cache: Arc<Mutex<DesktopEntryCache>>,
    scroll_speed: i32,
    scroll_bar_height: u32,
    history_icon: Svg,
}

impl ListView {
    pub fn create(
        display: &Display,
        signal_sender: Sender<Signal>,
        layout: ListViewLayout,
        visual_info: &XVisualInfo,
        colormap: Colormap,
        cache: Arc<Mutex<DesktopEntryCache>>,
        config: &Config,
    ) -> Self {
        let window = Window::builder(display)
            .size(layout.window.width, layout.window.height)
            .attributes(|attributes| {
                attributes
                    .colormap(colormap)
                    .border_pixel(0)
                    .background_pixel(colors::BACKGROUND.pack());
            })
            .visual(visual_info.visual)
            .depth(visual_info.depth)
            .build();
        let mut dc = DrawingContext::create(
            display,
            layout.window.width,
            layout.item_height * CAPACITY,
            visual_info,
        );
        dc.set_font(&FontDescription::from_string(&config.list_font));
        // Since the items in the main drawing context are only rendered once we
        // need separate contexts for dynamic visuals.
        let empty_screen = create_empty_screen(
            display,
            layout.window.width,
            layout.window.height,
            visual_info,
            &config.list_empty_font,
        );
        Self {
            window,
            display: *display,
            signal_sender,
            layout,
            dc,
            items: Vec::new(),
            scroll: 0,
            max_scroll_offset: 0,
            selected: 0,
            click_item: usize::MAX,
            click_time: 0,
            search: String::new(),
            empty_screen,
            cache,
            scroll_speed: config.scroll_speed,
            scroll_bar_height: 0,
            history_icon: Svg::load(resources::HISTORY_ICON),
        }
    }

    fn position_to_item_index(&self, offset: i32) -> usize {
        (offset as u32 / self.layout.item_height) as usize
    }

    fn item_index_to_position(&self, idx: usize) -> i32 {
        (idx as u32 * self.layout.item_height) as i32
    }

    pub fn set_items<T: Render + 'static>(&mut self, items: &[T], search: &str, no_draw: bool) {
        self.items = items
            .iter()
            .map(|x| {
                let as_static: &'static _ = unsafe { &*(x as *const T) };
                LazyItem::new(as_static)
            })
            .collect();
        if self.items.is_empty() {
            if !no_draw {
                self.draw();
            }
            return;
        }
        let visible = (self.layout.window.height / self.layout.item_height) as i32;
        self.max_scroll_offset =
            (self.items.len() as i32 - visible) * self.layout.item_height as i32;
        self.max_scroll_offset = self.max_scroll_offset.clamp(
            0,
            (CAPACITY * self.layout.item_height - self.layout.window.height) as i32,
        );
        self.dc.fill(colors::BACKGROUND);
        self.search = search.to_string();
        // TODO: if previously selected is in new list, keep it selected
        self.scroll = 0;
        self.selected = 0;
        self.resize_scrollbar();
        if !no_draw {
            self.draw();
        }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn resize_scrollbar(&mut self) {
        if self.layout.scroll_bar_width == 0 {
            return;
        }
        let visible_height = self.layout.window.height;
        let content_height = self.items.len() as u32 * self.layout.item_height;
        if content_height <= visible_height {
            self.scroll_bar_height = 0;
            return;
        }
        let percentage = visible_height as f64 / content_height as f64;
        self.scroll_bar_height = (visible_height as f64 * percentage).round() as u32;
    }

    fn draw_scrollbar(&mut self) {
        if self.scroll_bar_height == 0 || self.layout.scroll_bar_width == 0 {
            return;
        }
        let visible_height = self.layout.window.height;
        let pos = self.scroll as f64 / self.max_scroll_offset as f64;
        let y = ((visible_height - self.scroll_bar_height) as f64 * pos) as i32;
        // Redraw visible item backgrounds below scrollbar area.
        let first_visible = self.position_to_item_index(self.scroll);
        let last_visible =
            self.position_to_item_index(self.scroll + self.layout.window.height as i32);
        for idx in first_visible..=last_visible {
            let rect = Rectangle::new(
                (self.layout.window.width - self.layout.scroll_bar_width) as i32,
                self.item_index_to_position(idx),
                self.layout.scroll_bar_width,
                self.layout.item_height,
            );
            self.dc
                .rect(&rect)
                .color(if idx == self.selected {
                    colors::LIST_SELECTED_BACKGROUND
                } else if idx % 2 == 0 {
                    colors::BACKGROUND
                } else {
                    colors::LIST_LIGHT_BACKGROUND
                })
                .draw();
        }
        // Draw the scrollbar
        let rect = Rectangle::new(
            (self.layout.window.width - self.layout.scroll_bar_width) as i32,
            self.scroll + y,
            self.layout.scroll_bar_width,
            self.scroll_bar_height,
        );
        self.dc
            .rect(&rect)
            .color(colors::LIST_SCROLL_BAR)
            .corner_radius(0.499)
            .draw();
    }

    #[inline]
    fn draw_item(&mut self, idx: usize, redraw: bool) {
        let i = &mut self.items[idx];
        if redraw || !i.is_rendered() {
            let (background, icon, mut text) = self.layout.get_item_rects(idx);
            self.dc
                .rect(&background)
                // XXX: this will always ebe highlit, indicating it would be
                // enter action, even if pressing enter would interact with the
                // smart content.
                .color(if idx == self.selected {
                    colors::LIST_SELECTED_BACKGROUND
                } else if idx % 2 == 0 {
                    colors::BACKGROUND
                } else {
                    colors::LIST_LIGHT_BACKGROUND
                })
                .draw();
            let item = i.get(&self.search, &self.cache);
            if let Some(svg) = &item.icon {
                self.dc.svg(svg, &icon);
            }
            if item.is_in_history {
                let icon = ListViewLayout::add_secondary_icon(&mut text);
                self.dc
                    .colored_svg(&mut self.history_icon, colors::LIST_MATCH_NAME, &icon);
            }
            self.dc.set_color(colors::TEXT);
            self.dc
                .text(&item.markup_text, text, true)
                .center_height()
                .ellipsize(EllipsizeMode::End)
                .draw();
        }
    }

    pub fn draw(&mut self) {
        if self.items.is_empty() {
            self.empty_screen.render(
                self.window,
                &Rectangle::new(0, 0, self.layout.window.width, self.layout.window.height),
            );
            return;
        }
        for y in (self.scroll..(self.scroll + self.layout.window.height as i32))
            .step_by(self.layout.item_height as usize)
        {
            let idx = self.position_to_item_index(y);
            if idx == self.items.len() {
                break;
            }
            self.draw_item(idx, false);
        }
        let mut rect = self.layout.window;
        rect.y += self.scroll;
        self.draw_scrollbar();
        self.dc.render_to_00(self.window, &rect);
    }

    /// Moves the view so the selection is visible
    fn adjust_view(&mut self) {
        let sel_top = self.item_index_to_position(self.selected);
        let sel_bot = self.item_index_to_position(self.selected + 1);
        if sel_top < self.scroll {
            self.scroll = sel_top;
        } else if sel_bot >= self.scroll + self.layout.window.height as i32 {
            self.scroll = sel_bot - self.layout.window.height as i32;
        }
        self.scroll = self.scroll.clamp(0, self.max_scroll_offset);
        self.draw();
    }

    /// Moves the selection so it's inside the view
    fn adjust_selection(&mut self) {
        let min = self.position_to_item_index(self.scroll);
        let max = self.position_to_item_index(self.scroll + self.layout.window.height as i32 - 1);
        let selected = self.selected.clamp(min, max);
        if selected != self.selected {
            self.change_selected(selected);
        }
    }

    fn change_selected(&mut self, to: usize) {
        let before = self.selected;
        self.selected = to.min(CAPACITY as usize - 1);
        self.draw_item(before, true);
        self.draw_item(self.selected, true);
        self.click_item = usize::MAX;
    }

    pub fn key_press(&mut self, key: KeyEvent) {
        if self.items.is_empty() {
            match key.key {
                Key::Escape => send_signal(&self.display, &self.signal_sender, Signal::Quit),
                Key::Tab => send_signal(&self.display, &self.signal_sender, Signal::SwapFocus),
                _ => {}
            }
            return;
        }
        match key.key {
            Key::Down => {
                if self.selected < self.items.len() - 1 {
                    self.change_selected(self.selected + 1);
                    self.adjust_view();
                }
            }
            Key::Up => {
                if self.selected > 0 {
                    self.change_selected(self.selected - 1);
                    self.adjust_view();
                } else {
                    send_signal(&self.display, &self.signal_sender, Signal::SwapFocus);
                }
            }
            Key::Home => {
                if self.selected != 0 {
                    self.change_selected(0);
                    self.adjust_view();
                }
            }
            Key::End => {
                if self.selected != self.items.len() - 1 {
                    self.change_selected(self.items.len() - 1);
                    self.adjust_view();
                }
            }

            Key::Enter => send_signal(
                &self.display,
                &self.signal_sender,
                Signal::Commit(Some(self.selected)),
            ),
            Key::Escape => send_signal(&self.display, &self.signal_sender, Signal::Quit),
            Key::Tab => send_signal(&self.display, &self.signal_sender, Signal::SwapFocus),
            Key::Delete => {
                if !self.is_empty() {
                    send_signal(
                        &self.display,
                        &self.signal_sender,
                        Signal::DeleteEntry(self.selected),
                    );
                }
            }
            _ => {}
        }
    }

    pub fn hit_test(&self, x: i32, y: i32) -> bool {
        self.layout.window.at(self.layout.reparent).contains(x, y)
    }

    pub fn button_press(&mut self, event: &XButtonPressedEvent) {
        if self.items.is_empty() {
            return;
        }
        const MOUSE_WHEEL_UP: u32 = Button4;
        const MOUSE_WHEEL_DOWN: u32 = Button5;
        let redraw;
        let scroll_before = self.scroll;
        #[allow(non_upper_case_globals)]
        match event.button {
            MOUSE_WHEEL_UP => {
                self.scroll -= self.scroll_speed;
                if self.scroll < 0 {
                    self.scroll = 0;
                }
                redraw = self.scroll != scroll_before;
                if redraw {
                    self.adjust_selection();
                }
            }
            MOUSE_WHEEL_DOWN => {
                self.scroll += self.scroll_speed;
                // TODO: which one?
                //if self.scroll >= self.max_scroll_offset {
                //  self.scroll = self.max_scroll_offset - 1;
                //}
                if self.scroll > self.max_scroll_offset {
                    self.scroll = self.max_scroll_offset;
                }
                redraw = self.scroll != scroll_before;
                if redraw {
                    self.adjust_selection();
                }
            }
            Button1 => {
                let y = event.y - self.layout.reparent.1;
                let click_idx = self.position_to_item_index(self.scroll + y);
                if click_idx >= self.items.len() {
                    // We may have less items than the widget is high but will allow
                    // clicks anywhere on the widget.
                    return;
                }
                if click_idx != self.click_item {
                    self.change_selected(click_idx);
                    self.click_item = click_idx;
                    // This already redraws
                    self.adjust_view();
                } else if event.time - self.click_time < 500 {
                    send_signal(
                        &self.display,
                        &self.signal_sender,
                        Signal::Commit(Some(self.click_item)),
                    );
                }
                self.click_time = event.time;
                return;
            }
            _ => {
                return;
            }
        }
        if redraw {
            self.draw();
        }
    }
}

impl Drop for ListView {
    fn drop(&mut self) {
        self.dc.destroy();
    }
}
