use std::{fs::File, io::BufWriter, path::Path};

use arboard::{Clipboard, ImageData};
use slint::{Rgba8Pixel, SharedPixelBuffer};

pub fn save_png(
    img: &SharedPixelBuffer<Rgba8Pixel>,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut enc = png::Encoder::new(
        BufWriter::new(File::create(path)?),
        img.width(),
        img.height(),
    );
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header()?;
    w.write_image_data(img.as_bytes())?;
    w.finish()?;
    Ok(())
}

/// Hidden first argument that runs this binary as a clipboard server.
pub const SERVE_ARG: &str = "--serve-clipboard";

/// Puts `img` on the clipboard.
///
/// On Linux a copy is only pasteable while some process serves it, and on
/// Wayland our window lingers (frozen) until this process exits. So we hand
/// the pixels to a detached copy of ourselves that serves them until
/// something else is copied, and this process is free to exit.
#[cfg(target_os = "linux")]
pub fn copy(img: &SharedPixelBuffer<Rgba8Pixel>) -> std::io::Result<()> {
    use std::{
        io::Write,
        os::unix::process::CommandExt,
        process::{Command, Stdio},
    };
    let mut child = Command::new(std::env::current_exe()?)
        .args([
            SERVE_ARG,
            &img.width().to_string(),
            &img.height().to_string(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        // own process group: a Ctrl+C in the launching terminal doesn't kill it
        .process_group(0)
        .spawn()?;
    child.stdin.take().expect("piped").write_all(img.as_bytes())
    // dropping the pipe closes it; the child is left running on purpose
}

#[cfg(not(target_os = "linux"))]
pub fn copy(img: &SharedPixelBuffer<Rgba8Pixel>) -> Result<(), arboard::Error> {
    Clipboard::new()?.set_image(image_data(img)) // the OS keeps the copy
}

/// The `SERVE_ARG` side: reads `w`x`h` RGBA pixels from stdin and serves
/// them on the clipboard until it's replaced.
#[cfg(target_os = "linux")]
pub fn serve(mut args: impl Iterator<Item = String>) -> Result<(), Box<dyn std::error::Error>> {
    use arboard::SetExtLinux;
    use std::io::Read;
    let mut dim = || -> Result<usize, Box<dyn std::error::Error>> {
        Ok(args.next().ok_or("missing size")?.parse()?)
    };
    let (width, height) = (dim()?, dim()?);
    let mut bytes = vec![0; width * height * 4];
    std::io::stdin().lock().read_exact(&mut bytes)?;
    let img = ImageData {
        width,
        height,
        bytes: bytes.into(),
    };
    Clipboard::new()?.set().wait().image(img)?;
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn image_data(img: &SharedPixelBuffer<Rgba8Pixel>) -> ImageData<'_> {
    ImageData {
        width: img.width() as usize,
        height: img.height() as usize,
        bytes: std::borrow::Cow::Borrowed(img.as_bytes()),
    }
}
