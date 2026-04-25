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
    if x == 0 {
        return 0;
    }

    let mut last = pixel_at(frame, x, y);
    for ix in (0..x).rev() {
        let current = pixel_at(frame, ix, y);
        if diff(current, last) > threshold as u16 {
            return x - ix;
        }
        last = current;
    }

    x
}

fn scan_right(frame: &FrozenFrame, x: u32, y: u32, threshold: u8) -> u32 {
    if x + 1 >= frame.width {
        return 0;
    }

    let mut last = pixel_at(frame, x, y);
    for ix in (x + 1)..frame.width {
        let current = pixel_at(frame, ix, y);
        if diff(current, last) > threshold as u16 {
            return ix - x;
        }
        last = current;
    }

    frame.width - 1 - x
}

fn scan_up(frame: &FrozenFrame, x: u32, y: u32, threshold: u8) -> u32 {
    if y == 0 {
        return 0;
    }

    let mut last = pixel_at(frame, x, y);
    for iy in (0..y).rev() {
        let current = pixel_at(frame, x, iy);
        if diff(current, last) > threshold as u16 {
            return y - iy;
        }
        last = current;
    }

    y
}

fn scan_down(frame: &FrozenFrame, x: u32, y: u32, threshold: u8) -> u32 {
    if y + 1 >= frame.height {
        return 0;
    }

    let mut last = pixel_at(frame, x, y);
    for iy in (y + 1)..frame.height {
        let current = pixel_at(frame, x, iy);
        if diff(current, last) > threshold as u16 {
            return iy - y;
        }
        last = current;
    }

    frame.height - 1 - y
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
        width: left + right,
        height: up + down,
        left_edge_x: x.saturating_sub(left),
        right_edge_x: (x + right).min(frame.width.saturating_sub(1)),
        top_edge_y: y.saturating_sub(up),
        bottom_edge_y: (y + down).min(frame.height.saturating_sub(1)),
    }
}
