mod configuration;
mod editor;
mod raster;
mod shapes;
mod utils;

use configuration::args::{Input, parse_args};
use editor::Editor;
use shapes::{rectangle::Rectangle, stroke::Stroke};
use slint::Rgba8Pixel;
use std::{cell::RefCell, env, fmt::Write, rc::Rc};

use crate::shapes::Shape;

slint::include_modules!();

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
                    if let Some(stroke) = ed.history.pop() {
                        ed.redo.push(stroke);
                        ed.rebuild();
                        push_overlay(&win, &ed);
                    }
                }
                Action::Redo => {
                    if let Some(stroke) = ed.redo.pop() {
                        ed.rasterize(&stroke);
                        ed.history.push(stroke);
                        push_overlay(&win, &ed);
                    }
                }
                Action::Save => { /* ... */ }
                Action::Copy => { /* ... */ }
                Action::Exit => {
                    let _ = slint::quit_event_loop();
                }
                // paint tools: remember the current tool on the Rust side
                tool => {
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
            let mut ed = ed.borrow_mut();
            let col = c.get_brush_color();
            let tool = win.global::<Actions>().get_current_tool();
            match tool {
                Action::Brush => {
                    ed.current = Some(shapes::Shape::Stroke(Stroke {
                        points: vec![(x, y)],
                        color: Rgba8Pixel {
                            r: col.red(),
                            g: col.green(),
                            b: col.blue(),
                            a: col.alpha(),
                        },
                        radius: c.get_brush_radius(),
                    }));
                    ed.live_path = format!("M {x} {y} L {x} {y}");
                }
                Action::Rectangle => {
                    ed.current = Some(shapes::Shape::Rectangle(Rectangle {
                        a: (x, y),
                        b: (x, y),
                        color: Rgba8Pixel {
                            r: col.red(),
                            g: col.green(),
                            b: col.blue(),
                            a: col.alpha(),
                        },
                        radius: c.get_brush_radius(),
                    }));
                    let ed = &mut *ed;
                    let Some(shape) = ed.current.as_mut() else {
                        return;
                    };
                    shape.write_preview(&mut ed.live_path);
                }
                _ => {}
            }
            c.set_live_path(ed.live_path.as_str().into())
        }
    });

    canvas.on_extend({
        let (ed, win) = (editor.clone(), window.as_weak());
        move |x, y| {
            let mut guard = ed.borrow_mut();
            let ed = &mut *guard;
            let win = win.unwrap();
            let Some(shape) = ed.current.as_mut() else {
                return;
            };

            match shape {
                Shape::Stroke(stroke) => {
                    let &(last_x, last_y) = stroke.points.last().unwrap();
                    if (x - last_x).powi(2) + (y - last_y).powi(2) < 1.0 {
                        return;
                    }
                    stroke.points.push((x, y));
                    let _ = write!(ed.live_path, " L {x} {y}");
                }
                Shape::Rectangle(rect) => {
                    rect.b = (x, y);
                    shape.write_preview(&mut ed.live_path);
                }
            }
            win.global::<Canvas>()
                .set_live_path(ed.live_path.as_str().into());
        }
    });

    canvas.on_end({
        let (ed, win) = (editor.clone(), window.as_weak());
        move || {
            let mut ed = ed.borrow_mut();
            let Some(shape) = ed.current.take() else {
                return;
            };
            ed.rasterize(&shape);
            ed.history.push(shape);
            ed.redo.clear();
            push_overlay(&win, &ed);
            win.unwrap().global::<Canvas>().set_live_path("".into());
        }
    });

    let _ = window.run();

    Ok(())
}
