use anyhow::{anyhow, bail, Context, Result};
use image::{imageops, RgbaImage};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::io::AsFd;
use std::path::PathBuf;

use wayland_client::{
    protocol::{
        wl_buffer::{self, WlBuffer},
        wl_compositor::{self, WlCompositor},
        wl_output::{self, Transform, WlOutput},
        wl_registry::{self, WlRegistry},
        wl_shm::{self, WlShm},
        wl_shm_pool::{self, WlShmPool},
        wl_surface::{self, WlSurface},
    },
    Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum,
};
use wayland_protocols_wlr::{
    layer_shell::v1::client::{
        zwlr_layer_shell_v1::{self, Layer, ZwlrLayerShellV1},
        zwlr_layer_surface_v1::{self, KeyboardInteractivity, ZwlrLayerSurfaceV1},
    },
    screencopy::v1::client::{
        zwlr_screencopy_frame_v1::{self, ZwlrScreencopyFrameV1},
        zwlr_screencopy_manager_v1::{self, ZwlrScreencopyManagerV1},
    },
};

#[derive(Debug, Clone)]
pub struct FrozenFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl FrozenFrame {
    pub fn from_rgba_image(img: RgbaImage) -> Self {
        let (width, height) = img.dimensions();
        Self {
            width,
            height,
            rgba: img.into_raw(),
        }
    }

    pub fn load(&self, x: u32, y: u32) -> [u8; 4] {
        let idx = ((y * self.width + x) * 4) as usize;
        [
            self.rgba[idx],
            self.rgba[idx + 1],
            self.rgba[idx + 2],
            self.rgba[idx + 3],
        ]
    }
}

struct OutputData {
    proxy: WlOutput,
    name: Option<String>,
    transform: Transform,
}

struct FrameInfo {
    format: WEnum<wl_shm::Format>,
    width: u32,
    height: u32,
    stride: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrameStatus {
    Pending,
    Ready,
    Failed,
}

struct State {
    compositor: Option<WlCompositor>,
    layer_shell: Option<ZwlrLayerShellV1>,
    shm: Option<WlShm>,
    screencopy: Option<ZwlrScreencopyManagerV1>,
    outputs: HashMap<u32, OutputData>,
    // output probe
    surface: Option<WlSurface>,
    buffer: Option<WlBuffer>,
    entered_output_id: Option<u32>,
    output_name: Option<String>,
    done: bool,
    // screencopy
    frame_info: Option<FrameInfo>,
    buffer_done: bool,
    y_invert: bool,
    frame_status: FrameStatus,
}

impl State {
    fn new() -> Self {
        Self {
            compositor: None,
            layer_shell: None,
            shm: None,
            screencopy: None,
            outputs: HashMap::new(),
            surface: None,
            buffer: None,
            entered_output_id: None,
            output_name: None,
            done: false,
            frame_info: None,
            buffer_done: false,
            y_invert: false,
            frame_status: FrameStatus::Pending,
        }
    }

    fn try_resolve(&mut self) {
        if self.done {
            return;
        }
        if let Some(id) = self.entered_output_id {
            if let Some(data) = self.outputs.get(&id) {
                if let Some(name) = &data.name {
                    self.output_name = Some(name.clone());
                    self.done = true;
                }
            }
        }
    }
}

impl Dispatch<WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { name, interface, version } = event {
            match interface.as_str() {
                "wl_compositor" => {
                    state.compositor = Some(
                        registry.bind::<WlCompositor, _, _>(name, version.min(4), qh, ()),
                    );
                }
                "zwlr_layer_shell_v1" => {
                    state.layer_shell = Some(
                        registry.bind::<ZwlrLayerShellV1, _, _>(name, version.min(4), qh, ()),
                    );
                }
                "zwlr_screencopy_manager_v1" => {
                    state.screencopy = Some(
                        registry.bind::<ZwlrScreencopyManagerV1, _, _>(name, version.min(3), qh, ()),
                    );
                }
                "wl_shm" => {
                    state.shm = Some(registry.bind::<WlShm, _, _>(name, version.min(1), qh, ()));
                }
                "wl_output" => {
                    let proxy = registry.bind::<WlOutput, _, _>(name, version.min(4), qh, name);
                    state.outputs.insert(
                        name,
                        OutputData {
                            proxy,
                            name: None,
                            transform: Transform::Normal,
                        },
                    );
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<WlCompositor, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlCompositor,
        _: wl_compositor::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlShm, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlShm,
        _: wl_shm::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlShmPool, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlShmPool,
        _: wl_shm_pool::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlBuffer, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlBuffer,
        _: wl_buffer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlOutput, u32> for State {
    fn event(
        state: &mut Self,
        _: &WlOutput,
        event: wl_output::Event,
        registry_id: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_output::Event::Geometry { transform, .. } => {
                if let WEnum::Value(t) = transform {
                    if let Some(data) = state.outputs.get_mut(registry_id) {
                        data.transform = t;
                    }
                }
            }
            wl_output::Event::Name { name } => {
                if let Some(data) = state.outputs.get_mut(registry_id) {
                    data.name = Some(name);
                }
                state.try_resolve();
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwlrLayerShellV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZwlrLayerShellV1,
        _: zwlr_layer_shell_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrLayerSurfaceV1, ()> for State {
    fn event(
        state: &mut Self,
        layer_surface: &ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_layer_surface_v1::Event::Configure { serial, .. } = event {
            layer_surface.ack_configure(serial);

            if let Some(surf) = &state.surface {
                if let Some(buf) = &state.buffer {
                    surf.attach(Some(buf), 0, 0);
                    surf.damage_buffer(0, 0, 1, 1);
                }
                surf.commit();
            }
        }
    }
}

impl Dispatch<WlSurface, ()> for State {
    fn event(
        state: &mut Self,
        _: &WlSurface,
        event: wl_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_surface::Event::Enter { output } = event {
            for (id, data) in &state.outputs {
                if data.proxy == output {
                    state.entered_output_id = Some(*id);
                    break;
                }
            }
            state.try_resolve();
        }
    }
}

impl Dispatch<ZwlrScreencopyManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZwlrScreencopyManagerV1,
        _: zwlr_screencopy_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrScreencopyFrameV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ZwlrScreencopyFrameV1,
        event: zwlr_screencopy_frame_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use zwlr_screencopy_frame_v1::Event;
        match event {
            Event::Buffer { format, width, height, stride } => {
                state.frame_info = Some(FrameInfo {
                    format,
                    width,
                    height,
                    stride,
                });
            }
            Event::BufferDone => state.buffer_done = true,
            Event::Flags { flags } => {
                if let WEnum::Value(f) = flags {
                    state.y_invert = f.contains(zwlr_screencopy_frame_v1::Flags::YInvert);
                }
            }
            Event::Ready { .. } => state.frame_status = FrameStatus::Ready,
            Event::Failed => state.frame_status = FrameStatus::Failed,
            _ => {}
        }
    }
}

/// Unlinked file used as shared memory. XDG_RUNTIME_DIR is tmpfs on practically
/// every desktop, /tmp is the fallback.
fn shm_file(size: u64) -> Result<File> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let path = dir.join(format!("sruler-{}.shm", std::process::id()));
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)
        .context("failed to create temporary shm file")?;
    let _ = std::fs::remove_file(&path);
    file.set_len(size).context("failed to size shm file")?;
    Ok(file)
}

/// Opens a 1x1 layer surface without naming an output, so the compositor puts it
/// on the output with the pointer, then reads which output it entered.
fn detect_output(
    state: &mut State,
    eq: &mut EventQueue<State>,
    qh: &QueueHandle<State>,
) -> Result<u32> {
    let compositor = state
        .compositor
        .clone()
        .ok_or_else(|| anyhow!("compositor does not advertise wl_compositor"))?;
    let layer_shell = state.layer_shell.clone().ok_or_else(|| {
        anyhow!(
            "compositor does not support zwlr_layer_shell_v1, is this a wlroots-based compositor (sway, river, labwc, ...)?"
        )
    })?;
    let shm = state
        .shm
        .clone()
        .ok_or_else(|| anyhow!("compositor does not advertise wl_shm"))?;

    let shm_file = shm_file(4)?;
    let pool = shm.create_pool(shm_file.as_fd(), 4, qh, ());
    let buffer = pool.create_buffer(0, 1, 1, 4, wl_shm::Format::Argb8888, qh, ());
    pool.destroy();

    let surface = compositor.create_surface(qh, ());
    let layer_surface = layer_shell.get_layer_surface(
        &surface,
        None,
        Layer::Overlay,
        "sruler-probe".to_string(),
        qh,
        (),
    );
    layer_surface.set_size(1, 1);
    layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);

    state.buffer = Some(buffer);
    state.surface = Some(surface);
    state.surface.as_ref().unwrap().commit();

    for _ in 0..10 {
        eq.roundtrip(state)
            .context("wayland roundtrip failed during probe")?;
        if state.done {
            break;
        }
    }

    // The probe must be gone before the capture or it ends up in the screenshot.
    layer_surface.destroy();
    if let Some(surf) = state.surface.take() {
        surf.destroy();
    }
    if let Some(buf) = state.buffer.take() {
        buf.destroy();
    }
    eq.roundtrip(state)
        .context("wayland roundtrip failed after probe teardown")?;

    match (state.done, state.entered_output_id) {
        (true, Some(id)) => Ok(id),
        _ => Err(anyhow!(
            "could not detect which output the cursor is on; try moving the cursor before running sruler"
        )),
    }
}

/// Converts the wl_shm buffer into top-down RGBA. Alpha is forced opaque since
/// the X formats leave it undefined.
fn convert_to_rgba(
    data: &[u8],
    info: &FrameInfo,
    format: wl_shm::Format,
    y_invert: bool,
) -> Result<RgbaImage> {
    use wl_shm::Format as F;

    let w = info.width as usize;
    let h = info.height as usize;
    let stride = info.stride as usize;
    if stride < w * 4 || data.len() < stride * h {
        bail!("screencopy buffer is smaller than its advertised geometry");
    }

    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
        let src_y = if y_invert { h - 1 - y } else { y };
        let row = &data[src_y * stride..src_y * stride + w * 4];
        let dst = &mut out[y * w * 4..(y + 1) * w * 4];
        let pixels = row.chunks_exact(4).zip(dst.chunks_exact_mut(4));

        match format {
            F::Argb8888 | F::Xrgb8888 => {
                for (s, d) in pixels {
                    d.copy_from_slice(&[s[2], s[1], s[0], 255]);
                }
            }
            F::Abgr8888 | F::Xbgr8888 => {
                for (s, d) in pixels {
                    d.copy_from_slice(&[s[0], s[1], s[2], 255]);
                }
            }
            F::Argb2101010 | F::Xrgb2101010 => {
                for (s, d) in pixels {
                    let p = u32::from_le_bytes([s[0], s[1], s[2], s[3]]);
                    d.copy_from_slice(&[
                        ((p >> 22) & 0xff) as u8,
                        ((p >> 12) & 0xff) as u8,
                        ((p >> 2) & 0xff) as u8,
                        255,
                    ]);
                }
            }
            F::Abgr2101010 | F::Xbgr2101010 => {
                for (s, d) in pixels {
                    let p = u32::from_le_bytes([s[0], s[1], s[2], s[3]]);
                    d.copy_from_slice(&[
                        ((p >> 2) & 0xff) as u8,
                        ((p >> 12) & 0xff) as u8,
                        ((p >> 22) & 0xff) as u8,
                        255,
                    ]);
                }
            }
            other => bail!("unsupported screencopy pixel format {:?}", other),
        }
    }

    RgbaImage::from_raw(info.width, info.height, out)
        .ok_or_else(|| anyhow!("screencopy pixel buffer has the wrong length"))
}

/// The buffer comes in the output's native orientation. Undo the output
/// transform so the image matches what is on screen.
fn undo_output_transform(img: RgbaImage, transform: Transform) -> RgbaImage {
    match transform {
        Transform::_90 => imageops::rotate90(&img),
        Transform::_180 => imageops::rotate180(&img),
        Transform::_270 => imageops::rotate270(&img),
        Transform::Flipped => imageops::flip_horizontal(&img),
        Transform::Flipped90 => imageops::flip_horizontal(&imageops::rotate90(&img)),
        Transform::Flipped180 => imageops::flip_horizontal(&imageops::rotate180(&img)),
        Transform::Flipped270 => imageops::flip_horizontal(&imageops::rotate270(&img)),
        _ => img,
    }
}

fn grab_output(
    state: &mut State,
    eq: &mut EventQueue<State>,
    qh: &QueueHandle<State>,
    output_id: u32,
) -> Result<RgbaImage> {
    let manager = state.screencopy.clone().ok_or_else(|| {
        anyhow!("compositor does not support zwlr_screencopy_manager_v1, so the screen cannot be captured")
    })?;
    let shm = state
        .shm
        .clone()
        .ok_or_else(|| anyhow!("compositor does not advertise wl_shm"))?;
    let (output, transform) = {
        let data = state
            .outputs
            .get(&output_id)
            .ok_or_else(|| anyhow!("output vanished before capture"))?;
        (data.proxy.clone(), data.transform)
    };

    // overlay_cursor = 0, the pointer stays out of the capture.
    let frame = manager.capture_output(0, &output, qh, ());

    // Version 3 announces all buffer types and then sends buffer_done.
    while state.frame_info.is_none() || !(state.buffer_done || frame.version() < 3) {
        if state.frame_status == FrameStatus::Failed {
            bail!("compositor refused the screencopy request");
        }
        eq.blocking_dispatch(state)
            .context("wayland dispatch failed while waiting for buffer info")?;
    }

    let info = state
        .frame_info
        .take()
        .ok_or_else(|| anyhow!("screencopy sent no wl_shm buffer description"))?;
    let format = match info.format {
        WEnum::Value(f) => f,
        WEnum::Unknown(raw) => bail!("unknown screencopy pixel format {raw:#x}"),
    };

    let size = info.stride as u64 * info.height as u64;
    let mut file = shm_file(size)?;
    let pool = shm.create_pool(file.as_fd(), size as i32, qh, ());
    let buffer = pool.create_buffer(
        0,
        info.width as i32,
        info.height as i32,
        info.stride as i32,
        format,
        qh,
        (),
    );
    pool.destroy();

    frame.copy(&buffer);
    while state.frame_status == FrameStatus::Pending {
        eq.blocking_dispatch(state)
            .context("wayland dispatch failed while waiting for the frame")?;
    }
    buffer.destroy();
    frame.destroy();

    if state.frame_status == FrameStatus::Failed {
        bail!("compositor failed to copy the output");
    }

    let mut data = vec![0u8; size as usize];
    file.seek(SeekFrom::Start(0))
        .context("failed to seek shm file")?;
    file.read_exact(&mut data)
        .context("failed to read captured pixels")?;

    let img = convert_to_rgba(&data, &info, format, state.y_invert)?;
    Ok(undo_output_transform(img, transform))
}

pub fn capture_screen() -> Result<(FrozenFrame, String)> {
    let conn = Connection::connect_to_env().context("failed to connect to Wayland display")?;
    let mut eq = conn.new_event_queue::<State>();
    let qh = eq.handle();

    conn.display().get_registry(&qh, ());

    let mut state = State::new();
    eq.roundtrip(&mut state)
        .context("wayland roundtrip 1 failed")?;
    eq.roundtrip(&mut state)
        .context("wayland roundtrip 2 failed")?;

    let output_id = detect_output(&mut state, &mut eq, &qh)
        .context("output detection via wl_surface::enter failed")?;
    let output_name = state.output_name.clone().unwrap_or_default();

    let image = grab_output(&mut state, &mut eq, &qh, output_id)
        .context("screencopy capture failed")?;

    Ok((FrozenFrame::from_rgba_image(image), output_name))
}
