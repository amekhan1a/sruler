use anyhow::{anyhow, bail, Context, Result};
use image::DynamicImage;
use std::collections::HashMap;
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::io::AsFd;
use std::path::PathBuf;
use std::process::Command;

use wayland_client::{
    Connection, Dispatch, QueueHandle,
    protocol::{
        wl_buffer::{self, WlBuffer},
        wl_compositor::{self, WlCompositor},
        wl_output::{self, WlOutput},
        wl_registry::{self, WlRegistry},
        wl_shm::{self, WlShm},
        wl_shm_pool::{self, WlShmPool},
        wl_surface::{self, WlSurface},
    },
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{self, Layer, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{self, KeyboardInteractivity, ZwlrLayerSurfaceV1},
};

#[derive(Debug, Clone)]
pub struct FrozenFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl FrozenFrame {
    pub fn from_dynamic(img: DynamicImage) -> Self {
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        Self {
            width,
            height,
            rgba: rgba.into_raw(),
        }
    }

    pub fn from_path(path: PathBuf) -> Result<Self> {
        let img = image::open(&path)
            .with_context(|| format!("failed to decode screenshot at {}", path.display()))?;
        Ok(Self::from_dynamic(img))
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
}

struct ProbeState {
    compositor: Option<WlCompositor>,
    layer_shell: Option<ZwlrLayerShellV1>,
    shm: Option<WlShm>,
    outputs: HashMap<u32, OutputData>,
    surface: Option<WlSurface>,
    buffer: Option<WlBuffer>,
    entered_output_id: Option<u32>,
    output_name: Option<String>,
    done: bool,
}

impl ProbeState {
    fn new() -> Self {
        Self {
            compositor: None,
            layer_shell: None,
            shm: None,
            outputs: HashMap::new(),
            surface: None,
            buffer: None,
            entered_output_id: None,
            output_name: None,
            done: false,
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

impl Dispatch<WlRegistry, ()> for ProbeState {
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
                "wl_shm" => {
                    state.shm = Some(
                        registry.bind::<WlShm, _, _>(name, version.min(1), qh, ()),
                    );
                }
                "wl_output" => {
                    let proxy =
                        registry.bind::<WlOutput, _, _>(name, version.min(4), qh, name);
                    state.outputs.insert(name, OutputData { proxy, name: None });
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<WlCompositor, ()> for ProbeState {
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

impl Dispatch<WlShm, ()> for ProbeState {
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

impl Dispatch<WlShmPool, ()> for ProbeState {
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

impl Dispatch<WlBuffer, ()> for ProbeState {
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

impl Dispatch<WlOutput, u32> for ProbeState {
    fn event(
        state: &mut Self,
        _: &WlOutput,
        event: wl_output::Event,
        registry_id: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Name { name } = event {
            if let Some(data) = state.outputs.get_mut(registry_id) {
                data.name = Some(name);
            }
            state.try_resolve();
        }
    }
}

impl Dispatch<ZwlrLayerShellV1, ()> for ProbeState {
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

impl Dispatch<ZwlrLayerSurfaceV1, ()> for ProbeState {
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

impl Dispatch<WlSurface, ()> for ProbeState {
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

fn anon_shm_file() -> Result<std::fs::File> {
    let path = std::env::temp_dir()
        .join(format!("sruler-probe-{}.shm", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)
        .context("failed to create temporary shm file")?;
    let _ = std::fs::remove_file(&path);
    file.write_all(&[0u8; 4])
        .context("failed to write shm pixel")?;
    file.seek(SeekFrom::Start(0))
        .context("failed to seek shm file")?;
    Ok(file)
}

fn detect_output_name() -> Result<String> {
    let conn = Connection::connect_to_env()
        .context("failed to connect to Wayland display")?;
    let mut eq = conn.new_event_queue::<ProbeState>();
    let qh = eq.handle();

    conn.display().get_registry(&qh, ());

    let mut state = ProbeState::new();

    eq.roundtrip(&mut state)
        .context("wayland roundtrip 1 failed")?;
    eq.roundtrip(&mut state)
        .context("wayland roundtrip 2 failed")?;

    let compositor = state
        .compositor
        .as_ref()
        .ok_or_else(|| anyhow!("compositor does not advertise wl_compositor"))?;
    let layer_shell = state
        .layer_shell
        .as_ref()
        .ok_or_else(|| anyhow!(
            "compositor does not support zwlr_layer_shell_v1 — is this a wlroots-based compositor (sway, river, labwc, …)?"
        ))?;
    let shm = state
        .shm
        .as_ref()
        .ok_or_else(|| anyhow!("compositor does not advertise wl_shm"))?;

    let shm_file = anon_shm_file()?;
    let pool = shm.create_pool(shm_file.as_fd(), 4, &qh, ());
    let buffer =
        pool.create_buffer(0, 1, 1, 4, wl_shm::Format::Argb8888, &qh, ());
    pool.destroy();

    let surface = compositor.create_surface(&qh, ());
    let layer_surface = layer_shell.get_layer_surface(
        &surface,
        None, // null output means compositor picks (usually the one with the pointer)
        Layer::Overlay,
        "sruler-probe".to_string(),
        &qh,
        (),
    );
    layer_surface.set_size(1, 1);
    layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);

    state.buffer = Some(buffer);
    state.surface = Some(surface);

    state.surface.as_ref().unwrap().commit();

    for _ in 0..10 {
        eq.roundtrip(&mut state)
            .context("wayland roundtrip failed during probe")?;
        if state.done {
            break;
        }
    }

    layer_surface.destroy();
    if let Some(surf) = state.surface.take() {
        surf.destroy();
    }
    if let Some(buf) = state.buffer.take() {
        buf.destroy();
    }
    let _ = eq.flush();

    state.output_name.ok_or_else(|| {
        anyhow!(
            "could not detect which output the cursor is on; try moving the cursor before running sruler"
        )
    })
}

pub fn capture_screen() -> Result<(FrozenFrame, String)> {
    let output_name = detect_output_name()
        .context("output detection via wl_surface::enter failed")?;

    let tmp_path = std::env::temp_dir().join("sruler-capture.png");

    let status = Command::new("grim")
        .arg("-o")
        .arg(&output_name)
        .arg(&tmp_path)
        .status()
        .context("failed to spawn grim — is it installed? if not, please install it")?;

    if !status.success() {
        bail!(
            "grim exited with a non-zero status while capturing output {:?}",
            output_name
        );
    }

    let frame = FrozenFrame::from_path(tmp_path.clone())?;
    let _ = std::fs::remove_file(tmp_path);
    Ok((frame, output_name))
}
