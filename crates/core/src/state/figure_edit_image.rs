//! Bounded explicit local raster input; the existing native asset store owns bytes.
use super::AssetId;
use super::AssetRecord;
use std::{io::Read, path::Path};
pub(super) fn prepare_image(path: &Path, total: &mut usize) -> Result<AssetRecord, String> {
    if !path.is_absolute() {
        return Err("Image path must name an absolute local PNG/JPEG file.".into());
    }
    if !std::fs::metadata(path)
        .map_err(|e| format!("Image asset unavailable at {}: {e}", path.display()))?
        .is_file()
    {
        return Err("Image path must name a regular file.".into());
    }
    let mut file = std::fs::File::open(path)
        .map_err(|e| format!("Image asset unavailable at {}: {e}", path.display()))?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Image path must name a regular file.".into());
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    *total += bytes.len();
    if bytes.len() > 1024 * 1024 || *total > 4 * 1024 * 1024 {
        return Err("Images allow 1 MiB per file and 4 MiB per batch.".into());
    }
    let probe =
        plotium_io::image::probe(&bytes).map_err(|e| format!("Image asset unavailable: {e}"))?;
    if !matches!(
        probe.format,
        plotium_io::image::RasterFormat::Png | plotium_io::image::RasterFormat::Jpeg
    ) || probe.decoded_bytes > 4 * 1024 * 1024
    {
        return Err("Use PNG/JPEG with at most 4 MiB decoded RGBA8 per image.".into());
    }
    let decoded = plotium_io::image::decode_rgba8(&bytes, false)
        .map_err(|e| format!("Image asset unavailable: {e}"))?;
    Ok(AssetRecord {
        id: AssetId::new(),
        sha256: decoded.sha256,
        format: probe.format.name().to_ascii_lowercase(),
        pixel_size: [probe.width, probe.height],
        bytes,
    })
}
