use std::io::Read;

pub fn load_image_from_stdin() -> Result<slint::Image, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    std::io::stdin().lock().read_to_end(&mut bytes)?;

    if bytes.is_empty() {
        return Err("no image data in stdin".into());
    }
    Ok(slint::Image::load_from_data(&bytes, None)?)
}
