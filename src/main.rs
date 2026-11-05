use std::{env, fs};

slint::include_modules!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let filename: String = env::args()
        .nth(1)
        .expect("please run it with filename such as; swagshot image.png");

    let screenshot = slint::Image::load_from_path(std::path::Path::new(&filename))?;
    let screenshot_size = screenshot.size();

    let window = MainWindow::new()?;
    window.set_img(screenshot);
    window.window().set_size(slint::PhysicalSize::new(
        screenshot_size.width,
        screenshot_size.height,
    ));
    let _ = window.run();

    Ok(())
}
