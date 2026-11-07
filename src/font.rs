use egui::{Color32, FontId, Painter, Pos2, Vec2};

/// Tooltip text drawn with egui's bundled monospace font, so any
/// character the font covers works and columns line up in the help box.
#[derive(Debug, Clone, Copy)]
pub struct TooltipFont {
    pub size: f32,
}

impl TooltipFont {
    pub fn new(size: f32) -> Self {
        Self { size }
    }

    fn font_id(&self) -> FontId {
        FontId::monospace(self.size)
    }

    pub fn measure(&self, painter: &Painter, text: &str) -> Vec2 {
        painter
            .layout(text.to_owned(), self.font_id(), Color32::WHITE, f32::INFINITY)
            .size()
    }

    pub fn draw_text(&self, painter: &Painter, pos: Pos2, text: &str, color: Color32) {
        let galley = painter.layout(text.to_owned(), self.font_id(), color, f32::INFINITY);
        painter.galley(pos, galley, color);
    }
}
