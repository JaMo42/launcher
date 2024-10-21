use std::borrow::Cow;

/// This is the widget displaying the smart content, see content.rs for classification.
use crate::{
    config::Config,
    draw::DrawingContext,
    layout::{Rectangle, SmartContentLayout},
    res::{resources, Svg},
    ui::colors,
    units::Unit,
    x::{Display, Window},
};
use pango::FontDescription;
use x11::xlib::{Colormap, XVisualInfo};

pub enum ReadyContent {
    Error(String),
    Expression(f64),
    /// (result, from, to)
    #[allow(unused)]
    Conversion(f64, Unit, Unit),
    /// (kind (to pick icon), action, what)
    Action(Action, &'static str, String),
}

impl Default for ReadyContent {
    fn default() -> Self {
        Self::Expression(0.0)
    }
}

pub enum Action {
    Web,
    Path,
    Run,
}

#[derive(Debug)]
pub enum SmartContentCommitAction {
    Copy(String),
    OpenPath(String),
    OpenWeb(String),
    Run(String),
}

impl ReadyContent {
    fn commit(self) -> Option<SmartContentCommitAction> {
        match self {
            ReadyContent::Error(_) => None,
            ReadyContent::Expression(value) => {
                Some(SmartContentCommitAction::Copy(value.to_string()))
            }
            ReadyContent::Conversion(result, _, _) => {
                Some(SmartContentCommitAction::Copy(format!("{result}")))
            }
            ReadyContent::Action(kind, _, what) => match kind {
                Action::Web => Some(SmartContentCommitAction::OpenWeb(what)),
                Action::Path => Some(SmartContentCommitAction::OpenPath(what)),
                Action::Run => Some(SmartContentCommitAction::Run(what)),
            },
        }
    }
}

pub struct SmartContent {
    pub window: Window,
    dc: DrawingContext,
    content: ReadyContent,
    layout: SmartContentLayout,
    pub selected: bool,
    showing_copied: bool,
    web_icon: Svg,
    path_icon: Svg,
    run_icon: Svg,
    calculate_icon: Svg,
    conversion_icon: Svg,
    error_icon: Svg,
}

impl SmartContent {
    pub fn create(
        display: &Display,
        layout: SmartContentLayout,
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
        dc.set_font(&FontDescription::from_string(&config.smart_content_font));

        Self {
            window,
            dc,
            content: ReadyContent::default(),
            layout,
            selected: false,
            showing_copied: false,
            web_icon: Svg::load(&resources::LANGUAGE_ICON),
            path_icon: Svg::load(&resources::FOLDER_OPEN_ICON),
            run_icon: Svg::load(&resources::TERMINAL_ICON),
            calculate_icon: Svg::load(&resources::CALCULATE_ICON),
            conversion_icon: Svg::load(&resources::CONVERSION_PATH_ICON),
            error_icon: Svg::load(&resources::WARNING_ICON),
        }
    }

    pub fn set(&mut self, content: ReadyContent) {
        self.content = content;
        self.selected = false;
    }

    fn render_content(&mut self) -> Rectangle {
        let (icon, text): (&mut Svg, Cow<str>) = match &self.content {
            ReadyContent::Error(e) => (&mut self.error_icon, e.as_str().into()),
            ReadyContent::Expression(e) => (&mut self.calculate_icon, e.to_string().into()),
            ReadyContent::Conversion(result, _, to) => (
                &mut self.conversion_icon,
                format!("{:.6} {}", result, to).into(),
            ),
            ReadyContent::Action(kind, action, what) => (
                match kind {
                    Action::Web => &mut self.web_icon,
                    Action::Path => &mut self.path_icon,
                    Action::Run => &mut self.run_icon,
                },
                format!("{} {}", action, what).into(),
            ),
        };
        self.dc.colored_svg(icon, colors::TEXT, &self.layout.icon);
        self.dc
            .text(&text, self.layout.text, false)
            .center_height()
            .draw()
    }

    pub fn draw(&mut self) {
        self.dc.fill(colors::LIST_LIGHT_BACKGROUND);
        self.dc.set_color(colors::TEXT);
        let content_rect = self.render_content();
        if self.showing_copied {
            self.dc
                .text("Copied!", self.layout.window, false)
                .right_align()
                .center_height()
                .draw();
            self.showing_copied = false;
        }
        if self.selected {
            let rect = content_rect.pad(4);
            self.dc.blend(true);
            self.dc.rect(&rect).color(colors::ENTRY_SELECTION).draw();
            self.dc.blend(false);
        }
        self.dc.render(self.window, &self.layout.window);
    }

    pub fn hit_test(&self, x: i32, y: i32) -> bool {
        self.layout.window.at(self.layout.reparent).contains(x, y)
    }

    pub fn set_selected(&mut self, selected: bool) {
        let was_selected = self.selected;
        if selected {
            self.selected = !self.selected;
        } else {
            self.selected = false;
        }
        if was_selected {
            self.showing_copied = true;
        }
        if self.selected != was_selected {
            self.draw();
        }
    }

    pub fn is_useful(&self) -> bool {
        !matches!(&self.content, ReadyContent::Error(_))
    }

    pub fn commit(&mut self) -> Option<SmartContentCommitAction> {
        std::mem::take(&mut self.content).commit()
    }
}
