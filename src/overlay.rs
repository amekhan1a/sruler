use crate::{
    capture::FrozenFrame,
    config::Config,
    font::TinyBitmapFont,
    measure::{measure, Measurement},
};
use anyhow::Result;
use arboard::Clipboard;
use eframe::{egui, App, CreationContext, Frame, NativeOptions};
use egui::{Color32, Pos2, Rect, Stroke, TextureHandle, TextureOptions, Vec2};

fn logical_to_physical(pos: Pos2, ppp: f32) -> Pos2 {
    Pos2::new(pos.x * ppp, pos.y * ppp)
}

fn snap_physical_px(pos: Pos2) -> Pos2 {
    Pos2::new(pos.x.round(), pos.y.round())
}

fn pixel_center_to_logical(pos_px: Pos2, ppp: f32) -> Pos2 {
    Pos2::new((pos_px.x + 0.5) / ppp, (pos_px.y + 0.5) / ppp)
}

fn pixel_len_to_logical(len_px: f32, ppp: f32) -> f32 {
    len_px / ppp
}

pub fn run(frame: FrozenFrame) -> Result<()> {
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
        Box::new(move |cc| Ok(Box::new(SrulerApp::new(cc, frame)))),
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))
}

pub struct SrulerApp {
    frame: FrozenFrame,
    texture: Option<TextureHandle>,
    measurement: Measurement,
    cursor_px: Pos2,
    threshold: u8,
    last_cursor_px: Option<Pos2>,
    clipboard: Option<Clipboard>,
    should_close: bool,
    config: Config,
    font: TinyBitmapFont,
}

impl SrulerApp {
    pub fn new(cc: &CreationContext<'_>, frame: FrozenFrame) -> Self {
        let config = Config::default();
        let font = TinyBitmapFont::new(config.tooltip_scale);
        let texture = Some(load_texture(&cc.egui_ctx, &frame));
        let center = Pos2::new(frame.width as f32 / 2.0, frame.height as f32 / 2.0);
        let threshold = 24;
        let measurement = measure(&frame, center.x as u32, center.y as u32, threshold);

        Self {
            frame,
            texture,
            measurement,
            cursor_px: center,
            threshold,
            last_cursor_px: Some(center),
            clipboard: Clipboard::new().ok(),
            should_close: false,
            config,
            font,
        }
    }

    fn recompute(&mut self, cursor_px: Pos2) {
        let cursor_px = snap_physical_px(cursor_px);
        self.cursor_px = cursor_px;
        self.measurement = measure(
            &self.frame,
            cursor_px
                .x
                .clamp(0.0, (self.frame.width.saturating_sub(1)) as f32) as u32,
            cursor_px
                .y
                .clamp(0.0, (self.frame.height.saturating_sub(1)) as f32) as u32,
            self.threshold,
        );
    }

    fn copy_and_quit(&mut self, ctx: &egui::Context) {
        let text = format!("{} x {}", self.measurement.width, self.measurement.height);
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
        let ppp = ctx.pixels_per_point();

        let mut cursor_changed = false;
        let mut new_cursor = self.cursor_px;
        let mut threshold_changed = false;
        let mut copy_and_exit = false;

        ctx.input(|input| {
            if let Some(pos) = input.pointer.latest_pos().or_else(|| input.pointer.hover_pos()) {
                let pos = snap_physical_px(logical_to_physical(pos, ppp));
                if self.last_cursor_px.map(|prev| prev != pos).unwrap_or(true) {
                    new_cursor = pos;
                    cursor_changed = true;
                }
            }

            let scroll = input
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::MouseWheel { delta, .. } => Some(delta.y),
                    _ => None,
                })
                .sum::<f32>();

            if scroll.abs() > f32::EPSILON {
                let step = if scroll.is_sign_positive() { 2 } else { -2 };
                self.threshold = ((self.threshold as i16) + step as i16).clamp(1, 255) as u8;
                threshold_changed = true;
            }

            if input.key_pressed(egui::Key::Escape) {
                self.should_close = true;
            }

            if input.pointer.button_clicked(egui::PointerButton::Primary) {
                copy_and_exit = true;
            }
        });

        if cursor_changed {
            self.last_cursor_px = Some(new_cursor);
            self.recompute(new_cursor);
        }

        if threshold_changed {
            self.recompute(self.cursor_px);
        }

        if copy_and_exit {
            self.copy_and_quit(ctx);
        }

        if self.should_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn paint_background(&self, ui: &mut egui::Ui) {
        if let Some(texture) = &self.texture {
            let rect = ui.max_rect();
            ui.painter().image(
                texture.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    }

    fn draw_cap(painter: &egui::Painter, pos: Pos2, vertical: bool, color: Color32, ppp: f32) {
        let size = if vertical {
            Vec2::new(pixel_len_to_logical(1.0, ppp), pixel_len_to_logical(3.0, ppp))
        } else {
            Vec2::new(pixel_len_to_logical(3.0, ppp), pixel_len_to_logical(1.0, ppp))
        };
        painter.rect_filled(Rect::from_center_size(pos, size), 0.0, color);
    }

    fn paint_measurement(&self, ui: &mut egui::Ui) {
        let ppp = ui.ctx().pixels_per_point();
        let measurement = &self.measurement;
        let painter = ui.painter();

        let cursor = pixel_center_to_logical(self.cursor_px, ppp);

        let left = pixel_center_to_logical(
            Pos2::new(measurement.left_edge_x as f32, self.cursor_px.y),
            ppp,
        );
        let right = pixel_center_to_logical(
            Pos2::new(measurement.right_edge_x as f32, self.cursor_px.y),
            ppp,
        );
        let up = pixel_center_to_logical(
            Pos2::new(self.cursor_px.x, measurement.top_edge_y as f32),
            ppp,
        );
        let down = pixel_center_to_logical(
            Pos2::new(self.cursor_px.x, measurement.bottom_edge_y as f32),
            ppp,
        );

        let arm = Stroke::new(
            pixel_len_to_logical(self.config.scanline_width, ppp),
            self.config.scanline_color,
        );

        painter.line_segment([left, cursor], arm);
        painter.line_segment([cursor, right], arm);
        painter.line_segment([up, cursor], arm);
        painter.line_segment([cursor, down], arm);

        if self.config.center_dot_enabled {
            painter.circle_filled(
                cursor,
                pixel_len_to_logical(self.config.center_dot_radius, ppp),
                self.config.center_dot_color,
            );
        }

        let cap_color = self.config.scanline_color;
        Self::draw_cap(
            painter,
            pixel_center_to_logical(
                Pos2::new(measurement.left_edge_x.saturating_sub(1) as f32, self.cursor_px.y),
                ppp,
            ),
            true,
            cap_color,
            ppp,
        );

        Self::draw_cap(
            painter,
            pixel_center_to_logical(
                Pos2::new(
                    (measurement.right_edge_x + 1).min(self.frame.width.saturating_sub(1)) as f32,
                    self.cursor_px.y,
                ),
                ppp,
            ),
            true,
            cap_color,
            ppp,
        );

        Self::draw_cap(
            painter,
            pixel_center_to_logical(
                Pos2::new(self.cursor_px.x, measurement.top_edge_y.saturating_sub(1) as f32),
                ppp,
            ),
            false,
            cap_color,
            ppp,
        );

        Self::draw_cap(
            painter,
            pixel_center_to_logical(
                Pos2::new(
                    self.cursor_px.x,
                    (measurement.bottom_edge_y + 1).min(self.frame.height.saturating_sub(1)) as f32,
                ),
                ppp,
            ),
            false,
            cap_color,
            ppp,
        );

        let logical_w = (measurement.width as f32 / ppp).round() as u32;
        let logical_h = (measurement.height as f32 / ppp).round() as u32;

        let tooltip = format!(
            "{} x {} px\n{} x {} logical\nT {}",
            measurement.width, measurement.height,
            logical_w, logical_h,
            measurement.threshold,
        );
        let text_size = self.font.measure(&tooltip);
        let padding = Vec2::new(8.0, 6.0);
        let box_size = text_size + padding * 2.0;
        let rect = ui.max_rect();

        let mut pos = cursor + Vec2::new(16.0, 18.0);
        if pos.x + box_size.x > rect.right() {
            pos.x = cursor.x - 16.0 - box_size.x;
        }
        if pos.y + box_size.y > rect.bottom() {
            pos.y = cursor.y - 16.0 - box_size.y;
        }
        pos.x = pos.x.clamp(rect.left() + 2.0, rect.right() - box_size.x - 2.0);
        pos.y = pos.y.clamp(rect.top() + 2.0, rect.bottom() - box_size.y - 2.0);

        let box_rect = Rect::from_min_size(pos, box_size);
        painter.rect_filled(box_rect, self.config.tooltip_radius, self.config.tooltip_bg);
        painter.rect_stroke(
            box_rect,
            self.config.tooltip_radius,
            Stroke::new(1.0, self.config.tooltip_border),
            egui::StrokeKind::Inside,
        );
        self.font
            .draw_text(painter, pos + padding, &tooltip, self.config.tooltip_text);
    }
}

impl App for SrulerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut Frame) {
        self.handle_input(ui.ctx());

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show_inside(ui, |ui| {
                let rect = ui.max_rect();
                let response = ui.allocate_rect(rect, egui::Sense::hover());

                let ppp = ui.ctx().pixels_per_point();
                if let Some(pos) = response
                    .hover_pos()
                    .or_else(|| ui.ctx().input(|i| i.pointer.interact_pos()))
                {
                    let pos = snap_physical_px(logical_to_physical(pos, ppp));
                    if self.last_cursor_px.map(|prev| prev != pos).unwrap_or(true) {
                        self.last_cursor_px = Some(pos);
                        self.recompute(pos);
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
