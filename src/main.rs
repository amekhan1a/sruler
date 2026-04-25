mod capture;
mod config;
mod font;
mod measure;
mod overlay;

use anyhow::{bail, Context, Result};
use capture::FrozenFrame;
use std::env;

fn ensure_wayland() -> Result<()> {
    if env::var_os("WAYLAND_DISPLAY").is_some() {
        return Ok(());
    }

    if matches!(env::var("XDG_SESSION_TYPE").as_deref(), Ok("wayland")) {
        return Ok(());
    }

    bail!("sruler needs a wayland session!");
}

fn main() -> Result<()> {
    ensure_wayland()?;

    let rt = tokio::runtime::Runtime::new().context("failed to start async runtime")?;
    let frame: FrozenFrame = rt
        .block_on(capture::capture_screen())
        .context("screen capture failed")?;

    overlay::run(frame).context("failed to launch overlay")?;
    Ok(())
}
