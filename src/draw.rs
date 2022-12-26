use crate::{
  layout::Rectangle,
  res::Svg,
  x::{Display, Window},
};
use cairo::{Context, LinearGradient, Operator, Surface};
use cairo_sys::cairo_xlib_surface_create;
use pango::{EllipsizeMode, FontDescription, Layout};
use x11::xlib::{
  Drawable, XCopyArea, XCreateGC, XCreatePixmap, XFreeGC, XFreePixmap, XVisualInfo, GC,
};

pub struct DrawingContext {
  pixmap: Drawable,
  gc: GC,
  surface: Surface,
  context: Context,
  layout: Layout,
  display: Display,
}

impl DrawingContext {
  pub fn create (display: &Display, width: u32, height: u32, visual_info: &XVisualInfo) -> Self {
    let pixmap = unsafe {
      XCreatePixmap (
        display.as_raw (),
        display.root (),
        width,
        height,
        visual_info.depth as u32,
      )
    };
    let surface = unsafe {
      let raw = cairo_xlib_surface_create (
        display.as_raw (),
        pixmap,
        visual_info.visual,
        width as i32,
        height as i32,
      );
      Surface::from_raw_full (raw).unwrap ()
    };
    let context = Context::new (&surface).unwrap ();
    context.set_operator (Operator::Source);
    let layout = pangocairo::create_layout (&context);
    let gc = unsafe { XCreateGC (display.as_raw (), pixmap, 0, std::ptr::null_mut ()) };
    Self {
      pixmap,
      gc,
      surface,
      context,
      layout,
      display: *display,
    }
  }

  pub fn destroy (&mut self) {
    unsafe {
      // TODO: figure out why not.
      //cairo_surface_destroy (self.surface.to_raw_none ());
      XFreePixmap (self.display.as_raw (), self.pixmap);
      XFreeGC (self.display.as_raw (), self.gc);
    }
  }

  pub fn blend (&mut self, yay_or_nay: bool) {
    if yay_or_nay {
      self.context.set_operator (Operator::Over);
    } else {
      self.context.set_operator (Operator::Source);
    }
  }

  pub fn set_color (&self, color: Color) {
    let (r, g, b, a) = color.float_parts ();
    self.context.set_source_rgba (r, g, b, a);
  }

  pub fn rect (&mut self, rect: &Rectangle) -> ShapeBuilder {
    ShapeBuilder::new (
      &mut self.context,
      ShapeKind::Rectangle,
      rect.x,
      rect.y,
      rect.width,
      rect.height,
    )
  }

  pub fn set_font (&mut self, description: &FontDescription) {
    self.layout.set_font_description (Some (description));
  }

  pub fn text (&mut self, text: &str, rect: Rectangle, markup: bool) -> TextBuilder {
    if markup {
      self.layout.set_markup (text);
    } else {
      self.layout.set_text (text);
    }
    TextBuilder::new (self, rect)
  }

  pub fn layout (&self) -> &Layout {
    &self.layout
  }

  pub fn svg (&mut self, svg: &Svg, rect: &Rectangle) {
    svg
      .renderer
      .render_document (&self.context, &rect.as_cairo ())
      .unwrap ()
  }

  pub fn colored_svg (&mut self, svg: &mut Svg, color: Color, rect: &Rectangle) {
    if svg.pattern.is_none () {
      self.context.save ().unwrap ();
      self.context.push_group ();
      self.svg (svg, rect);
      svg.pattern = Some (self.context.pop_group ().unwrap ());
      self.context.restore ().unwrap ();
    }
    self.set_color (color);
    self
      .context
      .mask (svg.pattern.as_ref ().unwrap ())
      .unwrap ();
  }

  pub fn fill (&mut self, color: Color) {
    self.set_color (color);
    self.context.paint ().unwrap ();
  }

  pub fn render (&self, window: Window, rect: &Rectangle) {
    self.surface.flush ();
    unsafe {
      XCopyArea (
        self.display.as_raw (),
        self.pixmap,
        window.handle (),
        self.gc,
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        rect.x,
        rect.y,
      );
    }
    self.display.flush ();
    self.display.sync (false);
  }

  pub fn render_to_00 (&self, window: Window, rect: &Rectangle) {
    self.surface.flush ();
    unsafe {
      XCopyArea (
        self.display.as_raw (),
        self.pixmap,
        window.handle (),
        self.gc,
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        0,
        0,
      );
    }
    self.display.flush ();
    self.display.sync (false);
  }
}

pub enum ShapeKind {
  Rectangle,
}

#[derive(Copy, Clone)]
pub struct Color {
  pub red: u8,
  pub green: u8,
  pub blue: u8,
  pub alpha: u8,
}

impl Color {
  pub const fn new (red: u8, green: u8, blue: u8, alpha: u8) -> Self {
    Self {
      red,
      green,
      blue,
      alpha,
    }
  }

  pub const fn pack (&self) -> u64 {
    ((self.alpha as u64) >> 24)
      | ((self.blue as u64) << 16)
      | ((self.green as u64) << 8)
      | (self.red as u64)
  }

  pub const fn scale (&self, percent: u16) -> Self {
    let scaled_red = self.red as u16 * percent / 100;
    let scaled_green = self.green as u16 * percent / 100;
    let scaled_blue = self.blue as u16 * percent / 100;
    //let scaled_alpha = self.alpha as u16 * percent / 100;
    Self {
      red: if scaled_red > 255 {
        255
      } else {
        scaled_red as u8
      },
      green: if scaled_green > 255 {
        255
      } else {
        scaled_green as u8
      },
      blue: if scaled_blue > 255 {
        255
      } else {
        scaled_blue as u8
      },
      alpha: self.alpha,
    }
  }

  pub const fn with_alpha (&self, alpha: u8) -> Self {
    let mut this = *self;
    this.alpha = alpha;
    this
  }

  fn float_parts (&self) -> (f64, f64, f64, f64) {
    (
      self.red as f64 / 255.0,
      self.green as f64 / 255.0,
      self.blue as f64 / 255.0,
      self.alpha as f64 / 255.0,
    )
  }
}

impl std::fmt::Display for Color {
  fn fmt (&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write! (f, "#{:>02X}{:>02X}{:>02X}", self.red, self.green, self.blue)
  }
}

pub struct GradientSpec {
  start: Color,
  end: Color,
  start_point: (f64, f64),
  end_point: (f64, f64),
}

impl GradientSpec {
  pub fn new_vertical (top: Color, bottom: Color) -> Self {
    Self {
      start: top,
      end: bottom,
      start_point: (0.0, 0.0),
      end_point: (0.0, 1.0),
    }
  }
}

enum ColorKind {
  None,
  Solid (Color),
  Gradient (GradientSpec),
}

pub struct ShapeBuilder<'a> {
  kind: ShapeKind,
  context: &'a mut Context,
  x: f64,
  y: f64,
  width: f64,
  height: f64,
  color: ColorKind,
  stroke: Option<(u32, Color)>,
  corner_radius_percent: Option<f64>,
}

impl<'a> ShapeBuilder<'a> {
  pub fn new (
    context: &'a mut Context,
    kind: ShapeKind,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
  ) -> Self {
    Self {
      kind,
      context,
      x: x as f64,
      y: y as f64,
      width: width as f64,
      height: height as f64,
      color: ColorKind::None,
      stroke: None,
      corner_radius_percent: None,
    }
  }

  pub fn color (&mut self, color: Color) -> &mut Self {
    self.color = ColorKind::Solid (color);
    self
  }

  pub fn gradient (&mut self, spec: GradientSpec) -> &mut Self {
    self.color = ColorKind::Gradient (spec);
    self
  }

  pub fn stroke (&mut self, width: u32, color: Color) -> &mut Self {
    self.stroke = Some ((width, color));
    // Half the stroke lies outside the shape but we want to preserve the
    // bounding box so we shrink it.
    let outside = width as f64 / 2.0;
    self.x += outside;
    self.y += outside;
    self.width -= width as f64;
    self.height -= width as f64;
    self
  }

  /// Sets the corner radius to a percentage of the shorter side of the bounding
  /// box.
  pub fn corner_radius (&mut self, percent: f64) -> &mut Self {
    self.corner_radius_percent = Some (percent);
    self
  }

  pub fn draw (&self) {
    self.set_path ();
    self.set_color ();
    self.fill ();
  }

  fn set_path (&self) {
    match self.kind {
      ShapeKind::Rectangle => {
        if let Some (corner_radius_percent) = self.corner_radius_percent {
          let r = f64::min (self.width, self.height) * corner_radius_percent;
          self.context.new_sub_path ();
          self.context.arc (
            self.x + self.width - r,
            self.y + r,
            r,
            -90.0f64.to_radians (),
            0.0f64.to_radians (),
          );
          self.context.arc (
            self.x + self.width - r,
            self.y + self.height - r,
            r,
            0.0f64.to_radians (),
            90.0f64.to_radians (),
          );
          self.context.arc (
            self.x + r,
            self.y + self.height - r,
            r,
            90.0f64.to_radians (),
            180.0f64.to_radians (),
          );
          self.context.arc (
            self.x + r,
            self.y + r,
            r,
            180.0f64.to_radians (),
            270.0f64.to_radians (),
          );
          self.context.close_path ();
        } else {
          self
            .context
            .rectangle (self.x, self.y, self.width, self.height);
        }
      }
    }
  }

  fn set_color (&self) {
    match self.color {
      ColorKind::None => {}
      ColorKind::Solid (ref color) => {
        let (r, g, b, a) = color.float_parts ();
        self.context.set_source_rgba (r, g, b, a);
      }
      ColorKind::Gradient (ref spec) => {
        let gradient = LinearGradient::new (
          spec.start_point.0,
          spec.start_point.1,
          spec.end_point.0 * self.width,
          spec.end_point.1 * self.height,
        );
        let (r, g, b, a) = spec.start.float_parts ();
        gradient.add_color_stop_rgba (0.0, r, g, b, a);
        let (r, g, b, a) = spec.end.float_parts ();
        gradient.add_color_stop_rgba (1.0, r, g, b, a);
      }
    }
  }

  fn fill (&self) {
    if let Some ((stroke_width, ref stroke_color)) = self.stroke {
      self.context.fill_preserve ().unwrap ();
      let (r, g, b, _a) = stroke_color.float_parts ();
      self.context.set_source_rgb (r, g, b);
      self.context.set_line_width (stroke_width as f64);
      self.context.stroke ().unwrap ();
    } else {
      self.context.fill ().unwrap ();
    }
  }
}

pub struct TextBuilder<'a> {
  dc: &'a mut DrawingContext,
  rect: Rectangle,
}

impl<'a> TextBuilder<'a> {
  fn new (dc: &'a mut DrawingContext, rect: Rectangle) -> Self {
    Self { dc, rect }
  }

  pub fn ellipsize (self, mode: EllipsizeMode) -> Self {
    self
      .dc
      .layout
      .set_width (self.rect.width as i32 * pango::SCALE);
    self.dc.layout.set_ellipsize (mode);
    self
  }

  pub fn center_width (mut self) -> Self {
    let (mut width, _) = self.dc.layout.size ();
    width /= pango::SCALE;
    self.rect.x += (self.rect.width as i32 - width) / 2;
    self
  }

  pub fn center_height (mut self) -> Self {
    let (_, mut height) = self.dc.layout.size ();
    height /= pango::SCALE;
    self.rect.y += (self.rect.height as i32 - height) / 2;
    self
  }

  pub fn draw (self) {
    self
      .dc
      .context
      .move_to (self.rect.x as f64, self.rect.y as f64);
    pangocairo::show_layout (&self.dc.context, &self.dc.layout);
  }
}
