use crate::parser::Result;
use base64::{engine::general_purpose::STANDARD, Engine};
use std::io::Cursor;
const MAX_BYTES: usize = 2 * 1024 * 1024;
/// Convert a local image to a small, embedded PNG. Decoder limits reject image bombs.
pub fn from_bytes(bytes: &[u8]) -> Result<String> {
    if bytes.len() > MAX_BYTES {
        return Err("Favicon must be smaller than 2 MiB".into());
    }
    let mut reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let icon = reader.decode()?.thumbnail(32, 32);
    let mut png = Cursor::new(Vec::new());
    icon.write_to(&mut png, image::ImageFormat::Png)?;
    Ok(format!(
        "data:image/png;base64,{}",
        STANDARD.encode(png.into_inner())
    ))
}
pub fn pixels(uri: &str) -> Result<(usize, usize, Vec<u8>)> {
    if uri.len() > MAX_BYTES * 2 {
        return Err("Favicon data URI is too large".into());
    }
    let (header, encoded) = uri.split_once(',').ok_or("Expected an image data URI")?;
    if !header.starts_with("data:image/") || !header.ends_with(";base64") {
        return Err("Expected a base64 image data URI".into());
    }
    let normalized = from_bytes(&STANDARD.decode(encoded)?)?;
    let data = STANDARD.decode(normalized.split_once(',').unwrap().1)?;
    let image = image::load_from_memory(&data)?.to_rgba8();
    Ok((
        image.width() as usize,
        image.height() as usize,
        image.into_raw(),
    ))
}
