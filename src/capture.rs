use anyhow::{anyhow, Context, Result};
use ashpd::desktop::screenshot::Screenshot;
use image::DynamicImage;
use std::path::PathBuf;
use url::Url;

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
        let img = image::open(&path).with_context(|| format!("failed to decode screenshot at {}", path.display()))?;
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

pub async fn capture_screen() -> Result<FrozenFrame> {
    let response = Screenshot::request()
        .interactive(false)
        .modal(true)
        .send()
        .await
        .context("failed to request screenshot portal")?
        .response()
        .context("screenshot request was rejected or cancelled")?;

    let uri = response.uri().to_string();
    let url = Url::parse(&uri).map_err(|e| anyhow!("portal returned an invalid URI: {e}"))?;

    if url.scheme() != "file" {
        return Err(anyhow!("portal returned unsupported URI scheme: {}", url.scheme()));
    }

    let path = url
        .to_file_path()
        .map_err(|_| anyhow!("portal returned a file URI that could not be converted to a path"))?;

    FrozenFrame::from_path(path)
}
