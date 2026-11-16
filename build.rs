use flate2::{write::GzEncoder, Compression};
use std::{env, fs, io, path::{Path, PathBuf}};

const FILES: &[&str] = &[
    "Cargo.toml", "Cargo.lock", "README.md", "LICENSE", "PKGBUILD", "build.rs",
    "src/main.rs", "src/capture.rs", "src/config.rs",
    "src/font.rs", "src/measure.rs", "src/overlay.rs",
];

fn main() -> io::Result<()> {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let name = env::var("CARGO_PKG_NAME").unwrap();
    let prefix = format!("{name}-{}", env::var("CARGO_PKG_VERSION").unwrap());

    let mut tar = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::best()));
    for f in FILES {
        let p = root.join(f);
        let data = match fs::read(&p) {
            Ok(d) => d,
            Err(_) => {
                println!("cargo:warning={f} missing, not in --extract-source");
                continue;
            }
        };
        println!("cargo:rerun-if-changed={f}");
        let mut h = tar::Header::new_gnu();
        h.set_size(data.len() as u64);
        h.set_mode(0o644);
        h.set_uid(0);
        h.set_gid(0);
        h.set_mtime(0);
        tar.append_data(&mut h, Path::new(&prefix).join(f), &data[..])?;
    }
    let gz = tar.into_inner()?.finish()?;
    fs::write(Path::new(&env::var("OUT_DIR").unwrap()).join(format!("{name}.tar.gz")), gz)
}
