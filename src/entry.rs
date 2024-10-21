use crate::app::{send_signal, Signal};
use crate::config::Config;
use crate::draw::{Color, ColorKind, DrawingContext, GradientSpec};
use crate::input::{Key, KeyEvent};
use crate::layout::{EntryLayout, Rectangle};
use crate::res::*;
use crate::ui::colors;
use crate::util::{copy, paste};
use crate::x::{Display, Window};
use pango::{EllipsizeMode, FontDescription};
use std::sync::mpsc::Sender;
use x11::xlib::*;

pub struct Entry {
    pub window: Window,
    text: Vec<char>,
    character_positions: Vec<i32>,
    cursor_position: usize,
    /// If the selection is active, this is one side of it with the cursor
    /// position being the other, either can be the start or end.
    selection: Option<usize>,
    icon: Svg,
    layout: EntryLayout,
    dc: DrawingContext,
    pub display: Display,
    signal_sender: Sender<Signal>,
    pub is_focused: bool,
}

impl Entry {
    pub fn create(
        display: &Display,
        signal_sender: Sender<Signal>,
        layout: EntryLayout,
        visual_info: &XVisualInfo,
        colormap: Colormap,
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
            layout.window.height,
            visual_info,
        );
        dc.set_font(&FontDescription::from_string(&config.entry_font));
        Self {
            window,
            text: Vec::new(),
            character_positions: vec![0],
            cursor_position: 0,
            selection: None,
            icon: Svg::load(resources::SEARCH_ICON),
            layout,
            dc,
            display: *display,
            signal_sender,
            is_focused: true,
        }
    }

    pub fn text(&self) -> String {
        self.text.iter().collect()
    }

    fn selection_range(&self) -> Option<(usize, usize)> {
        self.selection.map(|sel| {
            let start = usize::min(sel, self.cursor_position);
            let end = usize::max(sel, self.cursor_position);
            (start, end)
        })
    }

    pub fn set_focused(&mut self, focused: bool) {
        if focused == self.is_focused {
            return;
        }
        self.is_focused = focused;
        self.draw();
        if focused {
            self.draw_cursor_and_selection();
        }
    }

    fn draw_box(&mut self) {
        const GRADIENT_TOP: Color = colors::ENTRY_FOCUSED_BORDER.scale(105);
        const GRADIENT_BOT: Color = colors::ENTRY_FOCUSED_BORDER.scale(95);
        self.dc.fill(colors::BACKGROUND);
        self.dc
            .rect(&self.layout.box_)
            .color(colors::ENTRY_BACKGROUND)
            .corner_radius(self.layout.corner_radius)
            .stroke(
                self.layout.stroke,
                if self.is_focused {
                    ColorKind::Gradient(GradientSpec::new_vertical(GRADIENT_TOP, GRADIENT_BOT))
                } else {
                    ColorKind::Solid(colors::ENTRY_NORMAL_BORDER)
                },
            )
            .draw();
    }

    pub fn draw(&mut self) {
        self.draw_box();
        self.dc
            .colored_svg(&mut self.icon, colors::TEXT, &self.layout.icon);
        let text = if self.text.is_empty() {
            self.dc.set_color(colors::ENTRY_PLACEHOLDER_TEXT);
            "Search".to_string()
        } else {
            self.dc.set_color(colors::TEXT);
            self.text()
        };
        self.dc
            .text(&text, self.layout.text, false)
            .center_height()
            .ellipsize(EllipsizeMode::Start)
            .draw();
        self.dc.render(self.window, &self.layout.window);
    }

    fn update_character_positions(&mut self) {
        self.character_positions.clear();
        self.character_positions.push(0);
        if !self.text.is_empty() {
            let mut it = self.dc.layout().iter();
            loop {
                let extents = it.char_extents();
                let x = (extents.x() + extents.width()) / pango::SCALE;
                self.character_positions.push(x);
                if !it.next_cluster() {
                    break;
                }
            }
        }
    }

    pub fn draw_cursor_and_selection(&mut self) {
        let x = self.character_positions[self.cursor_position];
        self.dc
            .rect(&Rectangle::new(
                self.layout.text.x + x,
                self.layout.cursor_y,
                self.layout.cursor_width,
                self.layout.cursor_height,
            ))
            .color(colors::ENTRY_CURSOR)
            .draw();

        if let Some(sel) = self.selection {
            let start = usize::min(sel, self.cursor_position);
            let end = usize::max(sel, self.cursor_position);
            let start = self.character_positions[start];
            let end = self.character_positions[end];
            self.dc.blend(true);
            self.dc
                .rect(&Rectangle::new(
                    self.layout.text.x + start,
                    self.layout.cursor_y,
                    (end - start) as u32,
                    self.layout.cursor_height,
                ))
                .color(colors::ENTRY_SELECTION)
                .draw();
            self.dc.blend(false);
        }

        self.dc.render(self.window, &self.layout.text);
    }

    fn text_changed(&mut self, draw: bool) {
        if draw {
            self.draw();
        }
        self.update_character_positions();
        send_signal(
            &self.display,
            &self.signal_sender,
            Signal::SearchTextChanged(self.text()),
        );
    }

    fn cursor_changed(&mut self) {
        self.draw_cursor_and_selection();
        let x = self.layout.reparent.0
            + self.layout.text.x
            + self.character_positions[self.cursor_position];
        let y = self.layout.reparent.1 + self.layout.text.y;
        send_signal(
            &self.display,
            &self.signal_sender,
            Signal::CursorPositionChanged((x, y)),
        );
    }

    pub fn text_input(&mut self, text: &str) {
        if self.selection.is_some() {
            if text == "(" || text == ")" {
                let sel = self.selection.unwrap();
                let begin = usize::min(sel, self.cursor_position);
                let end = usize::max(sel, self.cursor_position);
                self.text.insert(end, ')');
                self.text.insert(begin, '(');
                // This makes the cursor stay on the inside of the parentheses
                // at either end of the selection
                self.cursor_position += 1;
                self.selection = Some(sel + 1);
                self.text_changed(true);
                self.cursor_changed();
                return;
            } else {
                self.delete_selection();
            }
        }
        let is_at_end =
            self.text.is_empty() || self.cursor_position == self.character_positions.len() - 1;
        for c in text.chars() {
            if is_at_end {
                self.text.push(c);
            } else {
                self.text.insert(self.cursor_position, c);
                self.cursor_position += 1;
            }
        }
        self.text_changed(true);
        if is_at_end {
            self.cursor_position = self.character_positions.len() - 1;
        }
        self.cursor_changed();
    }

    fn jump(&self, left: bool) -> usize {
        fn scan(
            chars: &[char],
            current: Option<&char>,
            range: impl Iterator<Item = usize>,
            or: usize,
        ) -> usize {
            let cond = current.map(|c| !c.is_alphanumeric()).unwrap_or(true);
            for pos in range {
                if chars[pos].is_alphanumeric() == cond {
                    return pos;
                }
            }
            or
        }
        #[allow(clippy::collapsible_else_if)]
        if left {
            if self.cursor_position == 0 {
                self.cursor_position
            } else {
                let current = self.text.get(self.cursor_position - 1);
                scan(
                    &self.text,
                    current,
                    (0..self.cursor_position - 1).rev(),
                    usize::MAX,
                )
                .overflowing_add(1)
                .0
            }
        } else {
            if self.cursor_position == self.character_positions.len() - 1 {
                self.cursor_position
            } else {
                let current = self.text.get(self.cursor_position);
                let end = self.character_positions.len() - 1;
                scan(&self.text, current, self.cursor_position + 1..end, end)
            }
        }
    }

    fn delete_selection(&mut self) {
        if self.selection.is_none() {
            return;
        }
        let sel = self.selection.unwrap();
        let start = usize::min(sel, self.cursor_position);
        let size = usize::max(sel, self.cursor_position) - start;
        self.text.drain(start..(start + size));
        self.cursor_position = start;
        self.selection = None;
    }

    pub fn key_press(&mut self, event: KeyEvent) {
        if self.text.is_empty() {
            match event.key {
                Key::Escape | Key::CtrlC => {
                    send_signal(&self.display, &self.signal_sender, Signal::Quit)
                }
                Key::Tab | Key::Down => {
                    send_signal(&self.display, &self.signal_sender, Signal::SwapFocus)
                }
                Key::Enter => send_signal(&self.display, &self.signal_sender, Signal::Commit(None)),
                Key::CtrlV => {
                    let text = paste();
                    if !text.is_empty() {
                        self.text_input(&text);
                        self.draw();
                        self.text_changed(false);
                        if !self.text.is_empty()
                            && self.cursor_position >= self.character_positions.len()
                        {
                            self.cursor_position = self.character_positions.len() - 1;
                        }
                        self.cursor_changed();
                    }
                }
                _ => {}
            }
            return;
        }
        let mut text_changed = false;
        let mut keep_selection = false;
        if event.is_shift && event.is_text_cursor_movement() {
            if self.selection.is_none() {
                self.selection = Some(self.cursor_position);
            }
            keep_selection = true;
        }
        match event.key {
            Key::Backspace => {
                if self.selection.is_some() {
                    self.delete_selection();
                    text_changed = true;
                } else if self.cursor_position > 0 {
                    if event.is_ctrl {
                        let from = self.jump(true);
                        self.text.drain(from..self.cursor_position);
                        self.cursor_position = from;
                    } else {
                        self.text.remove(self.cursor_position - 1);
                        self.cursor_position -= 1;
                    }
                    text_changed = true;
                }
            }
            Key::Delete => {
                if self.selection.is_some() {
                    self.delete_selection();
                    text_changed = true;
                } else if self.cursor_position < self.text.len() {
                    if event.is_ctrl {
                        let to = self.jump(false);
                        self.text.drain(self.cursor_position..to);
                    } else {
                        self.text.remove(self.cursor_position);
                    }
                    text_changed = true;
                }
            }
            Key::Left => {
                if event.is_ctrl {
                    self.cursor_position = self.jump(true);
                } else if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                }
            }
            Key::Right => {
                if event.is_ctrl {
                    self.cursor_position = self.jump(false);
                } else if self.cursor_position < self.character_positions.len() - 1 {
                    self.cursor_position += 1;
                }
            }
            Key::Home => {
                self.cursor_position = 0;
            }
            Key::End => {
                self.cursor_position = self.character_positions.len() - 1;
            }
            Key::CtrlA => {
                self.selection = Some(0);
                self.cursor_position = self.text.len();
                keep_selection = true;
            }
            Key::CtrlV => {
                let text = paste();
                if !text.is_empty() {
                    self.text_input(&text);
                    text_changed = true;
                }
            }
            Key::CtrlC => {
                if let Some((start, end)) = self.selection_range() {
                    let text = self.text();
                    let text = &text[start..end];
                    copy(text);
                    keep_selection = true;
                } else {
                    self.text.clear();
                    self.cursor_position = 0;
                    text_changed = true;
                }
            }
            Key::CtrlX => {
                if let Some((start, end)) = self.selection_range() {
                    let text = self.text();
                    let text = &text[start..end];
                    copy(text);
                    self.text.drain(start..end);
                    self.cursor_position = start;
                    text_changed = true;
                }
            }
            Key::Escape => {
                send_signal(&self.display, &self.signal_sender, Signal::Quit);
                return;
            }
            Key::Down => {
                send_signal(&self.display, &self.signal_sender, Signal::SwapFocus);
                return;
            }
            Key::Enter => {
                send_signal(&self.display, &self.signal_sender, Signal::Commit(None));
                return;
            }
            Key::Tab => send_signal(&self.display, &self.signal_sender, Signal::SwapFocus),
            _ => {
                return;
            }
        }
        if !keep_selection {
            self.selection = None;
        }
        self.draw();
        if text_changed {
            self.text_changed(false);
            if !self.text.is_empty() && self.cursor_position >= self.character_positions.len() {
                self.cursor_position = self.character_positions.len() - 1;
            }
        }
        self.cursor_changed();
    }

    /// x, y are in window coordinates
    pub fn hit_test(&self, x: i32, y: i32) -> bool {
        self.layout.window.at(self.layout.reparent).contains(x, y)
    }
}

impl Drop for Entry {
    fn drop(&mut self) {
        self.dc.destroy();
    }
}
