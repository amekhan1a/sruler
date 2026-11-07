use crate::{
    capture::FrozenFrame,
    config::Config,
    font::TooltipFont,
    measure::{measure, Measurement},
};
use anyhow::Result;
use arboard::Clipboard;
use eframe::{egui, App, CreationContext, Frame, NativeOptions};
use egui::{Color32, Pos2, Rect, Stroke, TextureHandle, TextureOptions, Vec2};

/// Whole-screen zoom steps. Integer factors keep every source pixel the same size.
const ZOOM_LEVELS: [u32; 10] = [1, 2, 3, 4, 6, 8, 12, 16, 24, 32];

const HELP_TEXT: &str = "\
scroll            threshold up / down
shift + scroll    zoom in / out at the cursor
middle drag       pan while zoomed
left click        copy size and quit
shift + right     point to point: click A, click B, again to restart
ctrl              snap the p2p line straight
right click       cancel p2p
esc               cancel p2p, then quit
h or f1           toggle this help";

fn logical_to_physical(pos: Pos2, ppp: f32) -> Pos2 {
    Pos2::new(pos.x * ppp, pos.y * ppp)
}

/// The physical pixel that contains the position.
fn snap_physical_px(pos: Pos2) -> Pos2 {
    Pos2::new(pos.x.floor(), pos.y.floor())
}

fn pixel_len_to_logical(len_px: f32, ppp: f32) -> f32 {
    len_px / ppp
}

/// Inclusive pixel counts, so clicking the first and last pixel of an
/// element gives the same size the threshold mode reports.
fn p2p_dims(a: Pos2, b: Pos2) -> (u32, u32) {
    (
        (b.x - a.x).abs() as u32 + 1,
        (b.y - a.y).abs() as u32 + 1,
    )
}

/// Keeps the longer axis of a to b and flattens the other one.
fn snap_straight(a: Pos2, b: Pos2) -> Pos2 {
    if (b.x - a.x).abs() >= (b.y - a.y).abs() {
        Pos2::new(b.x, a.y)
    } else {
        Pos2::new(a.x, b.y)
    }
}

#[derive(Debug, Clone, Copy)]
struct P2p {
    a: Pos2,
    /// None while the second point still follows the cursor.
    b: Option<Pos2>,
}

pub fn run(frame: FrozenFrame, output_name: String) -> Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("sruler")
            .with_fullscreen(true)
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_resizable(false)
            .with_app_id("evanga.sruler.sruler"),
        vsync: true,
        multisampling: 0,
        depth_buffer: 0,
        stencil_buffer: 0,
        hardware_acceleration: eframe::HardwareAcceleration::Required,
        run_and_return: false,
        ..Default::default()
    };

    eframe::run_native(
        "SRuler",
        options,
        Box::new(move |cc| Ok(Box::new(SrulerApp::new(cc, frame, output_name)))),
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))
}

pub struct SrulerApp {
    frame: FrozenFrame,
    texture: Option<TextureHandle>,
    measurement: Measurement,
    /// Pointer position in physical screen pixels.
    pointer_screen_px: Pos2,
    /// Image pixel under the pointer.
    cursor_px: Pos2,
    threshold: u8,
    clipboard: Option<Clipboard>,
    should_close: bool,
    config: Config,
    font: TooltipFont,
    output_name: String,
    /// Index into ZOOM_LEVELS.
    zoom_idx: usize,
    /// screen_phys = image_px * zoom + view_offset. Always <= 0 on both axes.
    view_offset: Vec2,
    screen_phys: Vec2,
    /// Frame pixels per logical point, measured from the real window size each
    /// frame. Equals egui's pixels_per_point when the compositor scales the
    /// window as expected, and stays correct when it does not.
    px_per_pt: f32,
    p2p: Option<P2p>,
    ctrl_held: bool,
    show_help: bool,
}

impl SrulerApp {
    pub fn new(cc: &CreationContext<'_>, frame: FrozenFrame, output_name: String) -> Self {
        let config = Config::default();
        let font = TooltipFont::new(config.tooltip_font_size);
        let texture = Some(load_texture(&cc.egui_ctx, &frame));
        let center = Pos2::new((frame.width / 2) as f32, (frame.height / 2) as f32);
        let threshold = 24;
        let measurement = measure(&frame, center.x as u32, center.y as u32, threshold);
        let screen_phys = Vec2::new(frame.width as f32, frame.height as f32);
        let show_help = config.show_help_on_start;

        Self {
            frame,
            texture,
            measurement,
            pointer_screen_px: center,
            cursor_px: center,
            threshold,
            clipboard: Clipboard::new().ok(),
            should_close: false,
            config,
            font,
            output_name,
            zoom_idx: 0,
            view_offset: Vec2::ZERO,
            screen_phys,
            px_per_pt: cc.egui_ctx.pixels_per_point(),
            p2p: None,
            ctrl_held: false,
            show_help,
        }
    }

    fn zoom(&self) -> f32 {
        ZOOM_LEVELS[self.zoom_idx] as f32
    }

    fn clamp_offset(&self, off: Vec2) -> Vec2 {
        let z = self.zoom();
        let min_x = (self.screen_phys.x - self.frame.width as f32 * z).min(0.0);
        let min_y = (self.screen_phys.y - self.frame.height as f32 * z).min(0.0);
        Vec2::new(
            off.x.clamp(min_x, 0.0).round(),
            off.y.clamp(min_y, 0.0).round(),
        )
    }

    /// Image pixel index under a physical screen position.
    fn screen_to_image(&self, s: Pos2) -> Pos2 {
        let z = self.zoom();
        let x = ((s.x - self.view_offset.x) / z).floor();
        let y = ((s.y - self.view_offset.y) / z).floor();
        Pos2::new(
            x.clamp(0.0, self.frame.width.saturating_sub(1) as f32),
            y.clamp(0.0, self.frame.height.saturating_sub(1) as f32),
        )
    }

    /// Logical screen position of the center of an image pixel.
    fn image_center_to_screen(&self, p: Pos2, ppp: f32) -> Pos2 {
        let z = self.zoom();
        Pos2::new(
            ((p.x + 0.5) * z + self.view_offset.x) / ppp,
            ((p.y + 0.5) * z + self.view_offset.y) / ppp,
        )
    }

    /// Screen x (physical, whole pixels) of the left edge of image column x.
    fn boundary_x(&self, x: f32) -> f32 {
        x * self.zoom() + self.view_offset.x
    }

    /// Screen y (physical, whole pixels) of the top edge of image row y.
    fn boundary_y(&self, y: f32) -> f32 {
        y * self.zoom() + self.view_offset.y
    }

    /// Top-left of the one-pixel screen cell that marks the middle of an image
    /// pixel. At odd zoom this is the true middle, at even zoom the lower/right
    /// of the two middle cells, so lines always land on whole screen pixels.
    fn center_phys(&self, p: Pos2) -> Pos2 {
        let z = self.zoom();
        let h = (z / 2.0).floor();
        Pos2::new(
            p.x * z + h + self.view_offset.x,
            p.y * z + h + self.view_offset.y,
        )
    }

    /// Cap length in screen pixels, always odd so it centers on the arm.
    fn cap_len(&self) -> f32 {
        2.0 * self.zoom().min(6.0) + 1.0
    }

    /// Returns true when the snapped pointer position changed.
    fn set_pointer(&mut self, pos_logical: Pos2, ppp: f32) -> bool {
        let p = snap_physical_px(logical_to_physical(pos_logical, ppp));
        if p != self.pointer_screen_px {
            self.pointer_screen_px = p;
            true
        } else {
            false
        }
    }

    fn refresh_cursor(&mut self) {
        self.cursor_px = self.screen_to_image(self.pointer_screen_px);
        self.measurement = measure(
            &self.frame,
            self.cursor_px.x as u32,
            self.cursor_px.y as u32,
            self.threshold,
        );
    }

    /// Steps the zoom and keeps the image point under the pointer in place.
    fn step_zoom(&mut self, dir: i32) {
        let last = ZOOM_LEVELS.len() as i32 - 1;
        let new_idx = (self.zoom_idx as i32 + dir).clamp(0, last) as usize;
        if new_idx == self.zoom_idx {
            return;
        }
        let old = self.zoom();
        let s = self.pointer_screen_px;
        let ux = (s.x - self.view_offset.x) / old;
        let uy = (s.y - self.view_offset.y) / old;
        self.zoom_idx = new_idx;
        let z = self.zoom();
        self.view_offset = self.clamp_offset(Vec2::new(s.x - ux * z, s.y - uy * z));
    }

    /// (first point, second point, second point locked), both in image pixels.
    fn p2p_endpoints(&self) -> Option<(Pos2, Pos2, bool)> {
        self.p2p.map(|p| match p.b {
            Some(b) => (p.a, b, true),
            None => {
                let here = self.cursor_px;
                let b = if self.ctrl_held {
                    snap_straight(p.a, here)
                } else {
                    here
                };
                (p.a, b, false)
            }
        })
    }

    fn advance_p2p(&mut self) {
        self.p2p = match self.p2p_endpoints() {
            // Nothing yet, or a finished measurement: start a new one.
            None | Some((_, _, true)) => Some(P2p {
                a: self.cursor_px,
                b: None,
            }),
            Some((a, b, false)) => Some(P2p { a, b: Some(b) }),
        };
    }

    fn copy_and_quit(&mut self, ctx: &egui::Context) {
        let (w, h) = match self.p2p_endpoints() {
            Some((a, b, _)) => p2p_dims(a, b),
            None => (self.measurement.width, self.measurement.height),
        };
        let text = format!("{} x {}", w, h);
        if let Some(clipboard) = self.clipboard.as_mut() {
            if let Err(err) = clipboard.set_text(text.clone()) {
                eprintln!("sruler: clipboard write failed: {err}");
            }
        } else {
            ctx.copy_text(text);
        }
        self.should_close = true;
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        let ppp = self.px_per_pt;

        let mut pointer_pos: Option<Pos2> = None;
        let mut pan = Vec2::ZERO;
        let mut scroll_threshold = 0.0f32;
        let mut scroll_zoom = 0.0f32;
        let mut ctrl = false;
        let mut escape = false;
        let mut toggle_help = false;
        let mut left_click = false;
        let mut right_click = false;
        let mut shift_right_click = false;

        ctx.input(|input| {
            pointer_pos = input.pointer.latest_pos().or_else(|| input.pointer.hover_pos());

            if input.pointer.button_down(egui::PointerButton::Middle) {
                pan = input.pointer.delta();
            }

            for event in &input.events {
                if let egui::Event::MouseWheel { delta, modifiers, .. } = event {
                    if modifiers.shift {
                        // Some stacks turn shift+wheel into horizontal scroll, so take both axes.
                        scroll_zoom += delta.x + delta.y;
                    } else {
                        scroll_threshold += delta.y;
                    }
                }
            }

            ctrl = input.modifiers.ctrl;
            escape = input.key_pressed(egui::Key::Escape);
            toggle_help =
                input.key_pressed(egui::Key::H) || input.key_pressed(egui::Key::F1);
            left_click = input.pointer.button_clicked(egui::PointerButton::Primary);

            if input.pointer.button_clicked(egui::PointerButton::Secondary) {
                if input.modifiers.shift {
                    shift_right_click = true;
                } else {
                    right_click = true;
                }
            }
        });

        let mut dirty = false;

        if let Some(pos) = pointer_pos {
            dirty |= self.set_pointer(pos, ppp);
        }

        if pan != Vec2::ZERO {
            let off = self.clamp_offset(self.view_offset + pan * ppp);
            if off != self.view_offset {
                self.view_offset = off;
                dirty = true;
            }
        }

        if scroll_zoom.abs() > f32::EPSILON {
            self.step_zoom(if scroll_zoom.is_sign_positive() { 1 } else { -1 });
            dirty = true;
        }

        if scroll_threshold.abs() > f32::EPSILON {
            let step: i16 = if scroll_threshold.is_sign_positive() { 2 } else { -2 };
            self.threshold = ((self.threshold as i16) + step).clamp(1, 255) as u8;
            dirty = true;
        }

        self.ctrl_held = ctrl;

        if dirty {
            self.refresh_cursor();
        }

        if toggle_help {
            self.show_help = !self.show_help;
        }

        if escape {
            // First Escape drops an active point-to-point measurement, the next one quits.
            if self.p2p.take().is_none() {
                self.should_close = true;
            }
        }

        if right_click {
            self.p2p = None;
        }

        if shift_right_click {
            self.advance_p2p();
        }

        if left_click {
            self.copy_and_quit(ctx);
        }

        if self.should_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn paint_background(&self, ui: &mut egui::Ui) {
        if let Some(texture) = &self.texture {
            let rect = ui.max_rect();
            let uv = if self.zoom_idx == 0 {
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0))
            } else {
                // Only the visible slice of the image is sampled.
                let z = self.zoom();
                let fw = self.frame.width as f32;
                let fh = self.frame.height as f32;
                let x0 = -self.view_offset.x / z;
                let y0 = -self.view_offset.y / z;
                let x1 = x0 + self.screen_phys.x / z;
                let y1 = y0 + self.screen_phys.y / z;
                Rect::from_min_max(
                    Pos2::new(x0 / fw, y0 / fh),
                    Pos2::new(x1 / fw, y1 / fh),
                )
            };
            ui.painter().image(texture.id(), rect, uv, Color32::WHITE);
        }
    }

    /// Fills a rect given in physical screen pixels, snapped to whole pixels.
    fn fill_phys(
        painter: &egui::Painter,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        color: Color32,
        ppp: f32,
    ) {
        let x0 = x0.round();
        let y0 = y0.round();
        let x1 = x1.round().max(x0 + 1.0);
        let y1 = y1.round().max(y0 + 1.0);
        painter.rect_filled(
            Rect::from_min_max(Pos2::new(x0 / ppp, y0 / ppp), Pos2::new(x1 / ppp, y1 / ppp)),
            0.0,
            color,
        );
    }

    /// Threshold mode. Returns the tooltip text.
    ///
    /// The arms run from the outer edge of the first matching pixel to the outer
    /// edge of the last one, so their length equals the reported size at any zoom.
    /// The caps sit in the screen column or row just outside that span.
    fn paint_threshold(&self, painter: &egui::Painter, ppp: f32) -> String {
        let m = &self.measurement;
        let c = self.cursor_px;
        let color = self.config.scanline_color;

        let w = self.config.scanline_width.round().max(1.0);
        let half = ((w - 1.0) / 2.0).floor();
        let mid = self.center_phys(c);
        let row = mid.y - half;
        let col = mid.x - half;

        let x_l = self.boundary_x(m.left_edge_x as f32);
        let x_r = self.boundary_x(m.right_edge_x as f32 + 1.0);
        let y_t = self.boundary_y(m.top_edge_y as f32);
        let y_b = self.boundary_y(m.bottom_edge_y as f32 + 1.0);

        Self::fill_phys(painter, x_l, row, x_r, row + w, color, ppp);
        Self::fill_phys(painter, col, y_t, col + w, y_b, color, ppp);

        if self.config.center_dot_enabled {
            painter.circle_filled(
                Pos2::new((mid.x + 0.5) / ppp, (mid.y + 0.5) / ppp),
                pixel_len_to_logical(self.config.center_dot_radius, ppp),
                self.config.center_dot_color,
            );
        }

        // Caps stay inside the screen image when an edge is the image border.
        let fx0 = self.boundary_x(0.0);
        let fx1 = self.boundary_x(self.frame.width as f32);
        let fy0 = self.boundary_y(0.0);
        let fy1 = self.boundary_y(self.frame.height as f32);
        let len = self.cap_len();
        let cap_y = (row + w / 2.0 - len / 2.0).round();
        let cap_x = (col + w / 2.0 - len / 2.0).round();

        let left = (x_l - w).max(fx0);
        let right = x_r.min(fx1 - w);
        let top = (y_t - w).max(fy0);
        let bottom = y_b.min(fy1 - w);

        Self::fill_phys(painter, left, cap_y, left + w, cap_y + len, color, ppp);
        Self::fill_phys(painter, right, cap_y, right + w, cap_y + len, color, ppp);
        Self::fill_phys(painter, cap_x, top, cap_x + len, top + w, color, ppp);
        Self::fill_phys(painter, cap_x, bottom, cap_x + len, bottom + w, color, ppp);

        let logical_w = (m.width as f32 / ppp).round() as u32;
        let logical_h = (m.height as f32 / ppp).round() as u32;

        format!(
            "{}\n{} x {} px\n{} x {} logical\nthreshold {}",
            self.output_name, m.width, m.height, logical_w, logical_h, m.threshold,
        )
    }

    /// Point-to-point mode. No threshold involved, only the two clicked pixels.
    fn paint_p2p(
        &self,
        painter: &egui::Painter,
        ppp: f32,
        a: Pos2,
        b: Pos2,
        locked: bool,
    ) -> String {
        let color = self.config.p2p_color;
        let guide = color.gamma_multiply(0.4);
        let ca = self.center_phys(a);
        let cb = self.center_phys(b);

        // Right-angle guides, snapped so they stay crisp.
        Self::fill_phys(
            painter,
            ca.x.min(cb.x),
            ca.y,
            ca.x.max(cb.x) + 1.0,
            ca.y + 1.0,
            guide,
            ppp,
        );
        Self::fill_phys(
            painter,
            cb.x,
            ca.y.min(cb.y),
            cb.x + 1.0,
            ca.y.max(cb.y) + 1.0,
            guide,
            ppp,
        );

        let to_logical = |c: Pos2| Pos2::new((c.x + 0.5) / ppp, (c.y + 0.5) / ppp);
        painter.line_segment(
            [to_logical(ca), to_logical(cb)],
            Stroke::new(pixel_len_to_logical(self.config.p2p_width, ppp), color),
        );

        for c in [ca, cb] {
            Self::fill_phys(painter, c.x - 4.0, c.y, c.x + 5.0, c.y + 1.0, color, ppp);
            Self::fill_phys(painter, c.x, c.y - 4.0, c.x + 1.0, c.y + 5.0, color, ppp);
        }

        let (w, h) = p2p_dims(a, b);
        let logical_w = (w as f32 / ppp).round() as u32;
        let logical_h = (h as f32 / ppp).round() as u32;
        let dist = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();

        let state = if locked {
            " locked"
        } else if self.ctrl_held {
            " straight"
        } else {
            ""
        };

        format!(
            "{}\np2p{}\n{} x {} px\n{} x {} logical\ndist {:.1}",
            self.output_name, state, w, h, logical_w, logical_h, dist,
        )
    }

    fn draw_box(&self, painter: &egui::Painter, rect: Rect, text: &str, padding: Vec2) {
        painter.rect_filled(rect, self.config.tooltip_radius, self.config.tooltip_bg);
        painter.rect_stroke(
            rect,
            self.config.tooltip_radius,
            Stroke::new(1.0_f32, self.config.tooltip_border),
            egui::StrokeKind::Inside,
        );
        self.font
            .draw_text(painter, rect.min + padding, text, self.config.tooltip_text);
    }

    /// Draws the tooltip next to the cursor and returns its rect.
    fn paint_tooltip(
        &self,
        painter: &egui::Painter,
        screen: Rect,
        cursor: Pos2,
        text: &str,
    ) -> Rect {
        let padding = Vec2::new(8.0, 6.0);
        let box_size = self.font.measure(painter, text) + padding * 2.0;

        let mut pos = cursor + Vec2::new(16.0, 18.0);
        if pos.x + box_size.x > screen.right() {
            pos.x = cursor.x - 16.0 - box_size.x;
        }
        if pos.y + box_size.y > screen.bottom() {
            pos.y = cursor.y - 16.0 - box_size.y;
        }
        pos.x = pos.x.clamp(screen.left() + 2.0, screen.right() - box_size.x - 2.0);
        pos.y = pos.y.clamp(screen.top() + 2.0, screen.bottom() - box_size.y - 2.0);

        let box_rect = Rect::from_min_size(pos, box_size);
        self.draw_box(painter, box_rect, text, padding);
        box_rect
    }

    /// Help box in the first screen corner that stays clear of the cursor and the tooltip.
    fn paint_help(&self, painter: &egui::Painter, screen: Rect, cursor: Pos2, tooltip: Rect) {
        let padding = Vec2::new(10.0, 8.0);
        let size = self.font.measure(painter, HELP_TEXT) + padding * 2.0;
        let m = 8.0;
        let corners = [
            Pos2::new(screen.left() + m, screen.top() + m),
            Pos2::new(screen.right() - size.x - m, screen.top() + m),
            Pos2::new(screen.left() + m, screen.bottom() - size.y - m),
            Pos2::new(screen.right() - size.x - m, screen.bottom() - size.y - m),
        ];

        let mut chosen = Rect::from_min_size(corners[0], size);
        for pos in corners {
            let rect = Rect::from_min_size(pos, size);
            let keep_clear = rect.expand(12.0);
            if !keep_clear.contains(cursor) && !keep_clear.intersects(tooltip) {
                chosen = rect;
                break;
            }
        }

        self.draw_box(painter, chosen, HELP_TEXT, padding);
    }

    fn paint_measurement(&self, ui: &mut egui::Ui) {
        let ppp = self.px_per_pt;
        let screen = ui.max_rect();
        let painter = ui.painter();
        let cursor = self.image_center_to_screen(self.cursor_px, ppp);

        let mut text = match self.p2p_endpoints() {
            Some((a, b, locked)) => self.paint_p2p(painter, ppp, a, b, locked),
            None => self.paint_threshold(painter, ppp),
        };

        if self.zoom_idx > 0 {
            text.push_str(&format!("\nzoom {}x", ZOOM_LEVELS[self.zoom_idx]));
        }

        text.push_str("\npress h or f1 to toggle help");

        let tooltip_rect = self.paint_tooltip(painter, screen, cursor, &text);

        if self.show_help {
            self.paint_help(painter, screen, cursor, tooltip_rect);
        }
    }
}

impl App for SrulerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut Frame) {
        let rect = ui.max_rect();
        if rect.width() > 0.0 && rect.height() > 0.0 {
            self.px_per_pt = self.frame.width as f32 / rect.width();
            self.screen_phys = Vec2::new(self.frame.width as f32, rect.height() * self.px_per_pt);
        }
        let ppp = self.px_per_pt;

        self.handle_input(ui.ctx());

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show_inside(ui, |ui| {
                let rect = ui.max_rect();
                let response = ui.allocate_rect(rect, egui::Sense::hover());

                if let Some(pos) = response
                    .hover_pos()
                    .or_else(|| ui.ctx().input(|i| i.pointer.interact_pos()))
                {
                    if self.set_pointer(pos, ppp) {
                        self.refresh_cursor();
                    }
                }

                self.paint_background(ui);
                self.paint_measurement(ui);
            });
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        Color32::TRANSPARENT.to_normalized_gamma_f32()
    }
}

fn load_texture(ctx: &egui::Context, frame: &FrozenFrame) -> TextureHandle {
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [frame.width as usize, frame.height as usize],
        &frame.rgba,
    );
    ctx.load_texture("sruler-frozen-frame", image, TextureOptions::NEAREST)
}
