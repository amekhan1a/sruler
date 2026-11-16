mod capture;
mod config;
mod font;
mod measure;
mod overlay;

use anyhow::{bail, Context, Result};
use std::env;

static SOURCE: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/", env!("CARGO_PKG_NAME"), ".tar.gz"));

fn extract_source() -> Result<()> {
    use std::io::Write;
    let name = concat!(env!("CARGO_PKG_NAME"), "-", env!("CARGO_PKG_VERSION"), "-src.tar.gz");
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(name)
        .and_then(|mut f| f.write_all(SOURCE))
        .with_context(|| format!("cannot write {name}"))?;
    println!("wrote {name}");
    Ok(())
}

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
    if env::args().nth(1).as_deref() == Some("--extract-source") {
        return extract_source();
    }
    ensure_wayland()?;
    let (frame, output_name) = capture::capture_screen().context("screen capture failed")?;
    overlay::run(frame, output_name).context("failed to launch overlay")?;
    Ok(())
}
