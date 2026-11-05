mod configuration;
mod utils;

use configuration::args::{Input, parse_args};
use std::{env, fs};

slint::include_modules!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args()?;

    let screenshot = match args.input {
        Input::Path(path) => slint::Image::load_from_path(&path)?,
        Input::Stdin => utils::stdin::load_image_from_stdin()?,
    };

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
