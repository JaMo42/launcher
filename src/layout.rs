use crate::config::Config;
use pango::FontDescription;

#[derive(Copy, Clone, Debug)]
pub struct Rectangle {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rectangle {
    pub fn scale(&mut self, percent: u32) {
        let new_width = self.width * percent / 100;
        let new_height = self.height * percent / 100;
        self.x += (self.width as i32 - new_width as i32) / 2;
        self.y += (self.height as i32 - new_height as i32) / 2;
        self.width = new_width;
        self.height = new_height;
    }
}

impl Rectangle {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn as_cairo(&self) -> cairo::Rectangle {
        cairo::Rectangle::new(
            self.x as f64,
            self.y as f64,
            self.width as f64,
            self.height as f64,
        )
    }

    pub fn at(&self, p: (i32, i32)) -> Self {
        Self {
            x: self.x + p.0,
            y: self.y + p.1,
            ..*self
        }
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x
            && x < self.x + self.width as i32
            && y >= self.y
            && y < self.y + self.height as i32
    }

    pub fn pad(mut self, amount: i32) -> Self {
        self.x -= amount;
        self.y -= amount;
        self.width += 2 * amount as u32;
        self.height += 2 * amount as u32;
        self
    }
}

#[derive(Debug)]
struct LayoutBuilder {
    total: Rectangle,
    available: Rectangle,
}

impl LayoutBuilder {
    fn new(rect: Rectangle) -> Self {
        Self {
            total: rect,
            available: rect,
        }
    }

    fn margin(&mut self, margin: u32) {
        self.available = Rectangle {
            x: self.available.x + margin as i32,
            y: self.available.y + margin as i32,
            width: self.available.width - 2 * margin,
            height: self.available.height - 2 * margin,
        };
    }

    fn into_rect(self) -> Rectangle {
        self.total
    }

    /// Size available for a child with a 1:1 aspect ratio.
    fn available_square_size(&self) -> u32 {
        u32::min(self.available.width, self.available.height)
    }

    fn add_top_child(&mut self, size: u32, space: i32) -> LayoutBuilder {
        let top = Rectangle {
            x: self.available.x,
            y: self.available.y,
            width: self.available.width,
            height: size,
        };
        self.available.y += size as i32 + space;
        self.available.height -= size + space as u32;
        LayoutBuilder::new(top)
    }

    fn add_left_child(&mut self, size: u32, space: i32) -> LayoutBuilder {
        let left = Rectangle {
            x: self.available.x,
            y: self.available.y,
            width: size,
            height: self.available.height,
        };
        self.available.x += size as i32 + space;
        self.available.width -= size + space as u32;
        LayoutBuilder::new(left)
    }

    fn available(&mut self) -> LayoutBuilder {
        LayoutBuilder::new(self.available)
    }

    fn make_origin(&mut self) -> (i32, i32) {
        assert!(self.total.x == self.available.x);
        assert!(self.total.y == self.available.y);
        let result = (self.total.x, self.total.y);
        self.total.x = 0;
        self.total.y = 0;
        self.available.x = 0;
        self.available.y = 0;
        result
    }
}

pub struct Layout {
    pub window: Rectangle,
    pub entry: EntryLayout,
    pub full_list_view: ListViewLayout,
    pub reduced_list_view: ListViewLayout,
    pub smart_content: SmartContentLayout,
}

pub struct EntryLayout {
    pub reparent: (i32, i32),
    pub window: Rectangle,
    pub box_: Rectangle,
    pub icon: Rectangle,
    pub text: Rectangle,
    pub corner_radius: f64,
    pub stroke: u32,
    pub cursor_y: i32,
    pub cursor_height: u32,
    pub cursor_width: u32,
}

impl EntryLayout {
    fn new(mut entry: LayoutBuilder) -> Self {
        let reparent = entry.make_origin();
        let margin = 2;
        entry.margin(margin);
        let mut box_ = entry.available();
        box_.margin(margin);
        let icon = box_.add_left_child(box_.available_square_size(), 0);
        box_.available.x -= 4;
        box_.available.width -= 8;
        let text = box_.available();
        let cursor_height = text.total.height * 80 / 100;
        let cursor_y = text.total.y + (text.total.height - cursor_height) as i32 / 2;
        Self {
            reparent,
            window: entry.into_rect(),
            box_: box_.into_rect(),
            icon: icon.into_rect(),
            text: text.into_rect(),
            corner_radius: 0.2,
            stroke: 2,
            cursor_y,
            cursor_height,
            cursor_width: 3,
        }
    }
}

pub struct ListViewLayout {
    pub reparent: (i32, i32),
    pub window: Rectangle,
    pub icon: Rectangle,
    pub text: Rectangle,
    pub item_height: u32,
    pub scroll_bar_width: u32,
}

impl ListViewLayout {
    fn new(mut list_view: LayoutBuilder, config: &Config) -> Self {
        let reparent = list_view.make_origin();
        // Dummy item representing a single item, the actual background rect for
        // items is created in `get_item_rects`.
        let mut item = list_view.add_top_child(config.list_item_height, 0);
        item.available.y += 4;
        item.available.height -= 8;
        item.available.width -= config.scroll_bar_width;
        let icon = item.add_left_child(config.list_item_height, 4);
        let text = item.available();
        Self {
            reparent,
            window: list_view.into_rect(),
            icon: icon.into_rect(),
            text: text.into_rect(),
            item_height: config.list_item_height,
            scroll_bar_width: config.scroll_bar_width,
        }
    }

    pub fn get_item_rects(&self, idx: usize) -> (Rectangle, Rectangle, Rectangle) {
        let y = (idx as u32 * self.item_height) as i32;
        let background = Rectangle::new(0, y, self.window.width, self.item_height);
        let mut icon = self.icon;
        icon.y += y;
        let mut text = self.text;
        text.y += y;
        (background, icon, text)
    }

    pub fn add_secondary_icon(text: &mut Rectangle) -> Rectangle {
        text.width -= text.height;
        let mut icon = Rectangle::new(text.x + text.width as i32, text.y, text.height, text.height);
        icon.scale(70);
        icon
    }
}

#[derive(Debug)]
pub struct SmartContentLayout {
    // I don't know what made me use sub-windows for all the widgets when I
    // initially wrote this, but I'll stay in-line with it.
    pub reparent: (i32, i32),
    pub window: Rectangle,
    pub icon: Rectangle,
    pub text: Rectangle,
}

impl SmartContentLayout {
    fn new(mut smart_content: LayoutBuilder, real_height: u32) -> Self {
        let reparent = smart_content.make_origin();
        let icon_size = real_height;
        let mut icon = smart_content.add_left_child(icon_size, 0).into_rect();
        icon.y += (smart_content.total.height - icon_size) as i32 / 2;
        icon.height = icon_size;
        smart_content.add_left_child(0, 10);
        let text = smart_content.available().into_rect();
        Self {
            reparent,
            window: smart_content.into_rect(),
            icon,
            text,
        }
    }
}

impl Layout {
    pub fn window_size(screen_width: u32, screen_height: u32, config: &Config) -> (u32, u32) {
        (
            screen_width * config.window_width_percent / 100,
            screen_height * config.window_height_percent / 100,
        )
    }

    pub fn new(
        screen_width: u32,
        screen_height: u32,
        config: &Config,
        font_height: impl Fn(&FontDescription) -> i32,
    ) -> Self {
        let mut window = LayoutBuilder::new(Rectangle {
            x: 0,
            y: 0,
            width: screen_width * config.window_width_percent / 100,
            height: screen_height * config.window_height_percent / 100,
        });
        window.margin(10);
        let entry = window.add_top_child(config.entry_height, 10);
        let full_list_view = window.available();
        let mut smart_content = {
            let font = FontDescription::from_string(&config.smart_content_font);
            window.add_top_child(font_height(&font) as _, 0)
        };
        let reduced_list_view = window.available();
        let mut entry = EntryLayout::new(entry);
        let mut full_list_view = ListViewLayout::new(full_list_view, config);
        let mut reduced_list_view = ListViewLayout::new(reduced_list_view, config);

        entry.icon.scale(70);

        let full_list_view_height =
            full_list_view.window.height / config.list_item_height * config.list_item_height;
        window.total.height -= full_list_view.window.height - full_list_view_height;
        full_list_view.window.height = full_list_view_height;

        let reduced_list_view_height =
            reduced_list_view.window.height / config.list_item_height * config.list_item_height;
        let delta = (config.list_item_height as i32 - smart_content.total.height as i32).abs();
        reduced_list_view.window.height = reduced_list_view_height;
        reduced_list_view.reparent.1 += delta;

        let real_height = smart_content.total.height;
        smart_content.total.height += delta as u32;
        smart_content.available.height = smart_content.total.height;
        let smart_content = SmartContentLayout::new(smart_content, real_height);

        Self {
            window: window.into_rect(),
            entry,
            full_list_view,
            reduced_list_view,
            smart_content,
        }
    }
}
