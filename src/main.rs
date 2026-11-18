mod configuration;
mod editor;
mod raster;
mod shapes;
mod utils;

use configuration::args::{Input, parse_args};
use editor::Editor;
use shapes::{circle::Circle, rectangle::Rectangle, stroke::Stroke};
use slint::Rgba8Pixel;
use std::{cell::RefCell, env, rc::Rc};

use crate::shapes::{Point, Shape, Style};

slint::include_modules!();

fn to_px(c: slint::Color) -> Rgba8Pixel {
    Rgba8Pixel {
        r: c.red(),
        g: c.green(),
        b: c.blue(),
        a: c.alpha(),
    }
}

fn shape_for(tool: Action, p: Point, style: Style) -> Option<Shape> {
    Some(match tool {
        Action::Brush => Shape::Stroke(Stroke::new(p, style)),
        Action::Rectangle => Shape::Rectangle(Rectangle::new(p, style)),
        Action::Circle => Shape::Circle(Circle::new(p, style)),
        _ => return None,
    })
}

fn push_overlay(win: &slint::Weak<MainWindow>, ed: &Editor) {
    if let Some(win) = win.upgrade() {
        win.global::<Canvas>()
            .set_overlay(slint::Image::from_rgba8(ed.layer.buf.clone()));
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args()?;

    let screenshot = match args.input {
        Input::Path(path) => slint::Image::load_from_path(&path)?,
        Input::Stdin => utils::stdin::load_image_from_stdin()?,
    };

    let screenshot_size = screenshot.size();

    let window = MainWindow::new()?;
    let width = screenshot_size.width;
    let height = screenshot_size.height;

    window.set_img(screenshot);
    window
        .window()
        .set_size(slint::PhysicalSize::new(width, height));

    let actions = window.global::<Actions>();
    let editor = Rc::new(RefCell::new(Editor::new(width, height)));

    actions.on_dispatch({
        let (ed, win) = (editor.clone(), window.as_weak());
        move |action| {
            let mut ed = ed.borrow_mut();
            match action {
                Action::Undo => {
                    if ed.undo() {
                        push_overlay(&win, &ed);
                    }
                }
                Action::Redo => {
                    if ed.redo() {
                        push_overlay(&win, &ed);
                    }
                }
                Action::Save => { /* ... */ }
                Action::Copy => { /* ... */ }
                Action::Exit => {
                    let _ = slint::quit_event_loop();
                }
                // paint tools: remember the current tool on the Rust side
                _ => {
                    // self.current_tool = tool;
                }
            }
        }
    });

    let canvas = window.global::<Canvas>();

    canvas.on_begin({
        let (ed, win) = (editor.clone(), window.as_weak());
        move |x, y| {
            let win = win.unwrap();
            let c = win.global::<Canvas>();
            let style = Style {
                color: to_px(c.get_brush_color()),
                radius: c.get_brush_radius(),
            };
            let tool = win.global::<Actions>().get_current_tool();
            let Some(shape) = shape_for(tool, (x, y), style) else {
                return;
            };
            let mut ed = ed.borrow_mut();
            ed.begin(shape);
            c.set_live_path(ed.live_path.as_str().into())
        }
    });

    canvas.on_extend({
        let (ed, win) = (editor.clone(), window.as_weak());
        move |x, y| {
            let mut guard = ed.borrow_mut();
            let ed = &mut *guard;
            let win = win.unwrap();

            if ed.extend((x, y)) {
                win.global::<Canvas>()
                    .set_live_path(ed.live_path.as_str().into());
            }
        }
    });

    canvas.on_end({
        let (ed, win) = (editor.clone(), window.as_weak());
        move || {
            let mut ed = ed.borrow_mut();

            if ed.end() {
                push_overlay(&win, &ed);
                win.unwrap().global::<Canvas>().set_live_path("".into());
            }
        }
    });

    let _ = window.run();

    Ok(())
}
