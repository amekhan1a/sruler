use crate::capture::FrozenFrame;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Measurement {
    pub cursor_x: u32,
    pub cursor_y: u32,
    pub threshold: u8,
    pub left: u32,
    pub right: u32,
    pub up: u32,
    pub down: u32,
    pub width: u32,
    pub height: u32,
    pub left_edge_x: u32,
    pub right_edge_x: u32,
    pub top_edge_y: u32,
    pub bottom_edge_y: u32,
}

#[inline]
fn diff(a: [u8; 4], b: [u8; 4]) -> u16 {
    let dr = a[0].abs_diff(b[0]) as u16;
    let dg = a[1].abs_diff(b[1]) as u16;
    let db = a[2].abs_diff(b[2]) as u16;
    dr + dg + db
}

#[inline]
fn pixel_at(frame: &FrozenFrame, x: u32, y: u32) -> [u8; 4] {
    frame.load(x, y)
}

fn scan_left(frame: &FrozenFrame, x: u32, y: u32, threshold: u8) -> u32 {
    let origin = pixel_at(frame, x, y);
    for ix in (0..x).rev() {
        if diff(pixel_at(frame, ix, y), origin) > threshold as u16 {
            return x - ix;
        }
    }
    x + 1
}

fn scan_right(frame: &FrozenFrame, x: u32, y: u32, threshold: u8) -> u32 {
    let origin = pixel_at(frame, x, y);
    for ix in (x + 1)..frame.width {
        if diff(pixel_at(frame, ix, y), origin) > threshold as u16 {
            return ix - x;
        }
    }
    frame.width - x
}

fn scan_up(frame: &FrozenFrame, x: u32, y: u32, threshold: u8) -> u32 {
    let origin = pixel_at(frame, x, y);
    for iy in (0..y).rev() {
        if diff(pixel_at(frame, x, iy), origin) > threshold as u16 {
            return y - iy;
        }
    }
    y + 1
}

fn scan_down(frame: &FrozenFrame, x: u32, y: u32, threshold: u8) -> u32 {
    let origin = pixel_at(frame, x, y);
    for iy in (y + 1)..frame.height {
        if diff(pixel_at(frame, x, iy), origin) > threshold as u16 {
            return iy - y;
        }
    }
    frame.height - y
}

pub fn measure(frame: &FrozenFrame, cursor_x: u32, cursor_y: u32, threshold: u8) -> Measurement {
    let x = cursor_x.min(frame.width.saturating_sub(1));
    let y = cursor_y.min(frame.height.saturating_sub(1));

    let left = scan_left(frame, x, y, threshold);
    let right = scan_right(frame, x, y, threshold);
    let up = scan_up(frame, x, y, threshold);
    let down = scan_down(frame, x, y, threshold);

    Measurement {
        cursor_x: x,
        cursor_y: y,
        threshold,
        left,
        right,
        up,
        down,
        width: left + right - 1,
        height: up + down - 1,
        left_edge_x: x.saturating_sub(left - 1),
        right_edge_x: (x + right - 1).min(frame.width.saturating_sub(1)),
        top_edge_y: y.saturating_sub(up - 1),
        bottom_edge_y: (y + down - 1).min(frame.height.saturating_sub(1)),
    }
}
