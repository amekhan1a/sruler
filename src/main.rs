mod capture;
mod config;
mod font;
mod measure;
mod overlay;

use anyhow::{bail, Context, Result};
use std::env;

fn ensure_wayland() -> Result<()> {
    if env::var_os("WAYLAND_DISPLAY").is_some() {
        return Ok(());
    }
    if matches!(env::var("XDG_SESSION_TYPE").as_deref(), Ok("wayland")) {
        return Ok(());
    }
    bail!("sruler needs a Wayland session");
}

fn main() -> Result<()> {
    ensure_wayland()?;
    let (frame, output_name) = capture::capture_screen().context("screen capture failed")?;
    overlay::run(frame, output_name).context("failed to launch overlay")?;
    Ok(())
}
