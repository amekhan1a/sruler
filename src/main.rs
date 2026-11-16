mod capture;
mod config;
mod font;
mod measure;
mod overlay;

use anyhow::{bail, Context, Result};
use std::env;

static SOURCE: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/", env!("CARGO_PKG_NAME"), ".tar.gz"));

const HELP: &str = concat!(
    "sruler: on-screen ruler for Wayland\n",
    "\n",
    "USAGE\n",
    "  sruler                   Capture the screen and open the measuring overlay.\n",
    "                           Needs a Wayland session.\n",
    "\n",
    "OPTIONS\n",
    "  -h, --help               Show this help.\n",
    "  -V, --version            Show the version.\n",
    "  --extract-source         Write this program's source to\n",
    "                           ./", env!("CARGO_PKG_NAME"), "-", env!("CARGO_PKG_VERSION"), "-src.tar.gz\n",
    "                           (refuses to overwrite an existing file).\n",
);

fn extract_source() -> Result<()> {
    use std::io::Write;
    let name = concat!(env!("CARGO_PKG_NAME"), "-", env!("CARGO_PKG_VERSION"), "-src.tar.gz");
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true) // never clobber
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
    let args: Vec<String> = env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        [] => {}
        ["-h" | "--help"] => {
            print!("{HELP}");
            return Ok(());
        }
        ["-V" | "--version"] => {
            println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        ["--extract-source"] => return extract_source(),
        _ => {
            eprintln!("{}: unrecognized arguments: {}", env!("CARGO_PKG_NAME"), args.join(" "));
            eprintln!("Try '{} --help' for usage.", env!("CARGO_PKG_NAME"));
            std::process::exit(2);
        }
    }

    ensure_wayland()?;
    let (frame, output_name) = capture::capture_screen().context("screen capture failed")?;
    overlay::run(frame, output_name).context("failed to launch overlay")?;
    Ok(())
}
