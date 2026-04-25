use egui::{Color32, Painter, Pos2, Rect, Vec2};

#[derive(Debug, Clone, Copy)]
pub struct TinyBitmapFont {
    pub scale: f32,
    pub char_w: f32,
    pub char_h: f32,
    pub spacing: f32,
    pub line_gap: f32,
}

impl TinyBitmapFont {
    pub fn new(scale: f32) -> Self {
        Self {
            scale,
            char_w: 5.0 * scale,
            char_h: 7.0 * scale,
            spacing: 1.0 * scale,
            line_gap: 2.0 * scale,
        }
    }

    pub fn measure(&self, text: &str) -> Vec2 {
        let mut max_w: f32 = 0.0;
        let mut lines = 0usize;
        for line in text.lines() {
            lines += 1;
            let chars = line.chars().count() as f32;
            let w = if chars <= 0.0 {
                0.0
            } else {
                chars * (self.char_w + self.spacing) - self.spacing
            };
            max_w = max_w.max(w);
        }
        if lines == 0 {
            return Vec2::ZERO;
        }
        let h = lines as f32 * self.char_h + (lines.saturating_sub(1)) as f32 * self.line_gap;
        Vec2::new(max_w, h)
    }

    pub fn draw_text(&self, painter: &Painter, pos: Pos2, text: &str, color: Color32) {
        let mut y = pos.y;
        for line in text.lines() {
            self.draw_line(painter, Pos2::new(pos.x, y), line, color);
            y += self.char_h + self.line_gap;
        }
    }

    fn draw_line(&self, painter: &Painter, pos: Pos2, line: &str, color: Color32) {
        let mut x = pos.x;
        for ch in line.chars() {
            if ch != ' ' {
                self.draw_glyph(painter, Pos2::new(x, pos.y), ch, color);
            }
            x += self.char_w + self.spacing;
        }
    }

    fn draw_glyph(&self, painter: &Painter, pos: Pos2, ch: char, color: Color32) {
        let glyph = glyph_bits(ch);
        let pixel = self.scale.max(1.0);
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..5u8 {
                if (bits >> (4 - col)) & 1 == 1 {
                    let min = Pos2::new(pos.x + col as f32 * pixel, pos.y + row as f32 * pixel);
                    let max = Pos2::new(min.x + pixel, min.y + pixel);
                    painter.rect_filled(Rect::from_min_max(min, max), 0.0, color);
                }
            }
        }
    }
}

fn glyph_bits(ch: char) -> [u8; 7] {
    match ch {
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
        '3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110],
        '6' => [0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110],
        'T' | 't' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'X' | 'x' | '×' => [0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b01010, 0b10001],
        '.' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100],
        ':' => [0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100, 0b00000],
        '-' => [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000],
        '+' => [0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000],
        _ => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000],
    }
}
