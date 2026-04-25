use egui::Color32;

#[derive(Debug, Clone)]
pub struct Config {
    pub scanline_width: f32,
    pub scanline_color: Color32,
    pub center_dot_enabled: bool,
    pub center_dot_color: Color32,
    pub center_dot_radius: f32,
    pub tooltip_bg: Color32,
    pub tooltip_text: Color32,
    pub tooltip_border: Color32,
    pub tooltip_scale: f32,
    pub tooltip_radius: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            scanline_width: 1.0,
            scanline_color: Color32::from_rgba_unmultiplied(0, 255, 255, 220),
            center_dot_enabled: false,
            center_dot_color: Color32::from_rgba_unmultiplied(255, 80, 0, 255),
            center_dot_radius: 2.0,
            tooltip_bg: Color32::from_rgba_unmultiplied(10, 10, 10, 200),
            tooltip_text: Color32::WHITE,
            tooltip_border: Color32::from_rgba_unmultiplied(255, 255, 255, 50),
            tooltip_scale: 2.0,
            tooltip_radius: 0.0,
        }
    }
}
