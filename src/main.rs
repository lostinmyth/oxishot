mod configuration;
mod editor;
mod export;
mod raster;
mod shapes;
mod utils;

use configuration::args::{Input, parse_args};
use editor::{Bounds, Editor, SelectMode};
use shapes::{
    arrow::Arrow,
    circle::Circle,
    rectangle::Rectangle,
    stroke::Stroke,
    text::{self, Font, Text},
};
use parley::fontique::FamilyId;
use slint::{ModelRc, Rgba8Pixel, SharedString, VecModel};
use std::{
    cell::RefCell,
    path::Path,
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::shapes::{Frame, Point, Shape, Style};

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
        Action::Arrow => Shape::Arrow(Arrow::new(p, style)),
        _ => return None,
    })
}

/// How many used colors the color popover lists (two rows).
const USED_COLORS: usize = 18;

fn push_overlay(win: &slint::Weak<MainWindow>, ed: &Editor) {
    if let Some(win) = win.upgrade() {
        win.global::<Canvas>()
            .set_overlay(slint::Image::from_rgba8(ed.layer.buf.clone()));
        let used: Vec<_> = ed.used_colors(USED_COLORS).into_iter().map(to_color).collect();
        win.global::<Styling>()
            .set_used_colors(ModelRc::new(VecModel::from(used)));
    }
    push_selection(win, ed);
}

fn to_box(((x0, y0), (x1, y1)): Bounds) -> SelectionBox {
    SelectionBox {
        x: x0,
        y: y0,
        width: x1 - x0,
        height: y1 - y0,
        angle: 0.0,
    }
}

fn frame_box(f: Frame) -> SelectionBox {
    let ((cx, cy), (hx, hy)) = (f.center, f.half);
    SelectionBox {
        x: cx - hx,
        y: cy - hy,
        width: hx * 2.0,
        height: hy * 2.0,
        angle: f.angle.to_degrees(),
    }
}

fn to_color(p: Rgba8Pixel) -> slint::Color {
    slint::Color::from_argb_u8(p.a, p.r, p.g, p.b)
}

/// "#rgb", "#rgba", "#rrggbb" or "#rrggbbaa" ("#" optional).
fn parse_hex(s: &str) -> Option<slint::Color> {
    let s = s.trim();
    let s = s.strip_prefix('#').unwrap_or(s);
    let digit = |i: usize| u8::from_str_radix(s.get(i..i + 1)?, 16).ok();
    let pair = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
    let (r, g, b, a) = match s.len() {
        3 | 4 => {
            let d = |i| digit(i).map(|v| v * 17);
            (d(0)?, d(1)?, d(2)?, if s.len() == 4 { d(3)? } else { 255 })
        }
        6 | 8 => (pair(0)?, pair(2)?, pair(4)?, if s.len() == 8 { pair(6)? } else { 255 }),
        _ => return None,
    };
    Some(slint::Color::from_argb_u8(a, r, g, b))
}

/// The style new shapes get, as set in the style panel.
fn current_style(win: &MainWindow) -> Style {
    let s = win.global::<Styling>();
    let fill = to_px(s.get_fill());
    Style {
        color: to_px(s.get_stroke()),
        radius: s.get_radius(),
        fill: (fill.a > 0).then_some(fill),
        font: Font {
            family: text::family_id(&s.get_font_family()).unwrap_or_else(text::default_family),
            weight: s.get_font_weight() as f32,
            size: s.get_font_size(),
        },
    }
}

/// Shows `family` in the style panel with the weights it has, moving the
/// weight to the nearest of them.
fn push_family(s: &Styling, family: FamilyId) {
    let weights = text::weights(family);
    s.set_font_weight(text::nearest_weight(&weights, s.get_font_weight() as f32) as i32);
    s.set_font_family(text::family_name(family).into());
    let weights: Vec<i32> = weights.into_iter().map(i32::from).collect();
    s.set_font_weights(ModelRc::new(VecModel::from(weights)));
}

/// Shows the selection's style in the style panel: each field comes from
/// the first selected shape that uses it.
fn push_style(win: &MainWindow, ed: &Editor) {
    let s = win.global::<Styling>();
    let shapes: Vec<&Shape> = ed.selected.iter().map(|&i| &ed.shapes[i]).collect();
    s.set_has_selection(!shapes.is_empty());
    let fillable = shapes.iter().find(|sh| sh.fillable());
    let stroked = shapes.iter().find(|sh| sh.stroked());
    s.set_selection_fillable(fillable.is_some());
    s.set_selection_stroked(stroked.is_some());
    let text = shapes.iter().find(|sh| matches!(sh, Shape::Text(_)));
    s.set_selection_text(text.is_some());
    if let Some(sh) = shapes.first() {
        s.set_stroke(to_color(sh.style().color));
    }
    if let Some(sh) = fillable {
        s.set_fill(sh.style().fill.map_or(slint::Color::default(), to_color));
    }
    if let Some(sh) = stroked {
        s.set_radius(sh.style().radius);
    }
    if let Some(sh) = text {
        let f = sh.style().font;
        s.set_font_size(f.size);
        s.set_font_weight(f.weight as i32);
        push_family(&s, f.family);
    }
}

/// Mirrors the selection boxes, marquee and drag state into the UI.
fn push_selection(win: &slint::Weak<MainWindow>, ed: &Editor) {
    let Some(win) = win.upgrade() else { return };
    push_style(&win, ed);
    let c = win.global::<Canvas>();
    let boxes: Vec<_> = ed.selection_frames().into_iter().map(frame_box).collect();
    c.set_selection(ModelRc::new(VecModel::from(boxes)));
    c.set_has_marquee(ed.marquee().is_some());
    if let Some(m) = ed.marquee() {
        c.set_marquee(to_box(m));
    }
    c.set_dragging(ed.dragging());
    c.set_rotating(ed.rotating());
    c.set_scaling(ed.scaling());
    // the corners may have moved; the next hover recomputes these
    c.set_rotate_hover(false);
    c.set_scale_hover(false);
}

/// Starts a select gesture at `p` and mirrors it into the UI.
fn press(win: &slint::Weak<MainWindow>, ed: &mut Editor, p: Point, mode: SelectMode) {
    if ed.press(p, mode) {
        if let Some(w) = win.upgrade() {
            let c = w.global::<Canvas>();
            c.set_drag_offset(Vec2 { x: 0.0, y: 0.0 });
            c.set_drag_angle(0.0);
            c.set_drag_overlay(slint::Image::from_rgba8(ed.drag_layer.buf.clone()));
        }
        push_overlay(win, ed);
    } else {
        push_selection(win, ed);
    }
}

/// The file name the save dialog suggests: -o's, or a timestamped one.
fn default_name(output: Option<&Path>) -> String {
    if let Some(name) = output.and_then(|p| p.file_name()) {
        return name.to_string_lossy().into_owned();
    }
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    format!("swagshot-{secs}.png")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "linux")]
    {
        let mut argv = std::env::args().skip(1);
        if argv.next().as_deref() == Some(export::SERVE_ARG) {
            return export::serve(argv);
        }
    }
    let args = parse_args()?;

    let screenshot = match args.input {
        Input::Path(path) => slint::Image::load_from_path(&path)?,
        Input::Stdin => utils::stdin::load_image_from_stdin()?,
    };

    let screenshot_size = screenshot.size();
    let base = screenshot.to_rgba8().ok_or("could not decode the image")?;
    // -o only presets where the save dialog starts
    let output = args.output;

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
                Action::Delete => {
                    if ed.delete_selected() {
                        push_overlay(&win, &ed);
                    }
                }
                Action::SelectAll => {
                    ed.select_all();
                    push_selection(&win, &ed);
                }
                Action::Deselect => {
                    if ed.deselect() {
                        push_selection(&win, &ed);
                    }
                }
                Action::Save => {
                    // snapshot now; the dialog runs without blocking the UI
                    let img = raster::flatten(&base, &ed.layer.buf);
                    let mut dialog = rfd::AsyncFileDialog::new()
                        .add_filter("PNG image", &["png"])
                        .set_file_name(default_name(output.as_deref()));
                    if let Some(dir) = output.as_deref().and_then(|p| p.parent()) {
                        if !dir.as_os_str().is_empty() {
                            dialog = dialog.set_directory(dir);
                        }
                    }
                    if let Some(w) = win.upgrade() {
                        dialog = dialog.set_parent(&w.window().window_handle());
                    }
                    let _ = slint::spawn_local(async move {
                        let Some(file) = dialog.save_file().await else {
                            return;
                        };
                        let path = file.path();
                        match export::save_png(&img, path) {
                            Ok(()) => {
                                eprintln!("saved to {}", path.display());
                                let _ = slint::quit_event_loop();
                            }
                            Err(e) => eprintln!("could not save {}: {e}", path.display()),
                        }
                    });
                }
                Action::Copy => {
                    let img = raster::flatten(&base, &ed.layer.buf);
                    match export::copy(&img) {
                        Ok(()) => {
                            let _ = slint::quit_event_loop();
                        }
                        Err(e) => eprintln!("could not copy: {e}"),
                    }
                }
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
            let style = current_style(&win);
            let tool = win.global::<Actions>().get_current_tool();
            let Some(shape) = shape_for(tool, (x, y), style) else {
                return;
            };
            let mut ed = ed.borrow_mut();
            ed.begin(shape);
            c.set_live_path(ed.live_path.as_str().into())
        }
    });

    canvas.on_select_begin({
        let (ed, win) = (editor.clone(), window.as_weak());
        move |x, y, mode| {
            let mode = match mode {
                SelectionMode::Replace => SelectMode::Replace,
                SelectionMode::Add => SelectMode::Add,
                SelectionMode::Remove => SelectMode::Remove,
            };
            press(&win, &mut ed.borrow_mut(), (x, y), mode);
        }
    });

    // drawing tools: a press on the selection (e.g. the shape just drawn)
    // moves, scales or rotates it; anywhere else drops it to draw anew
    canvas.on_grab({
        let (ed, win) = (editor.clone(), window.as_weak());
        move |x, y| {
            let mut ed = ed.borrow_mut();
            if ed.grabs_selection((x, y)) {
                press(&win, &mut ed, (x, y), SelectMode::Replace);
                return true;
            }
            if ed.deselect() {
                push_selection(&win, &ed);
            }
            false
        }
    });

    canvas.on_extend({
        let (ed, win) = (editor.clone(), window.as_weak());
        move |x, y| {
            let mut guard = ed.borrow_mut();
            let ed = &mut *guard;
            let win = win.unwrap();

            if let Some((dx, dy)) = ed.drag_offset((x, y)) {
                win.global::<Canvas>()
                    .set_drag_offset(Vec2 { x: dx, y: dy });
                return;
            }
            if let Some(((px, py), angle)) = ed.rotate_to((x, y)) {
                let c = win.global::<Canvas>();
                c.set_drag_pivot(Vec2 { x: px, y: py });
                c.set_drag_angle(angle.to_degrees());
                push_selection(&win.as_weak(), ed); // boxes turn with the shapes
                return;
            }
            if ed.scale_to((x, y)) {
                let c = win.global::<Canvas>();
                c.set_drag_overlay(slint::Image::from_rgba8(ed.drag_layer.buf.clone()));
                push_selection(&win.as_weak(), ed); // boxes stretch with the shapes
                return;
            }
            if ed.marquee_to((x, y)) {
                push_selection(&win.as_weak(), ed);
                return;
            }

            if ed.extend((x, y)) {
                win.global::<Canvas>()
                    .set_live_path(ed.live_path.as_str().into());
            }
        }
    });

    canvas.on_end({
        let (ed, win) = (editor.clone(), window.as_weak());
        move |x, y| {
            let mut ed = ed.borrow_mut();

            let had_marquee = ed.marquee().is_some();
            if ed.release((x, y)) {
                push_overlay(&win, &ed);
                return;
            }
            if had_marquee {
                push_selection(&win, &ed);
                return;
            }

            if ed.end() {
                push_overlay(&win, &ed);
                win.unwrap().global::<Canvas>().set_live_path("".into());
            }
        }
    });

    canvas.on_hover({
        let (ed, win) = (editor.clone(), window.as_weak());
        move |x, y| {
            let Some(w) = win.upgrade() else { return };
            let c = w.global::<Canvas>();
            let ed = ed.borrow();
            let handle = ed.scale_handle((x, y));
            c.set_scale_hover(handle.is_some());
            if let Some(a) = handle {
                c.set_scale_cursor(a.to_degrees());
            }
            let zone = ed.rotate_zone((x, y));
            c.set_rotate_hover(zone.is_some());
            if let Some(a) = zone {
                c.set_rotate_cursor(a.to_degrees());
            }
        }
    });

    canvas.on_commit_text({
        let (ed, win) = (editor.clone(), window.as_weak());
        move |x, y, text| {
            let Some(w) = win.upgrade() else { return };
            let shape = Shape::Text(Text {
                pos: (x, y),
                text: text.to_string(),
                angle: 0.0,
                style: Style {
                    radius: 0.0,
                    fill: None,
                    ..current_style(&w)
                },
            });
            let mut ed = ed.borrow_mut();
            ed.commit(shape);
            push_overlay(&win, &ed);
        }
    });

    // style panel edits preview on the selection; commit makes them one undo step
    let styling = window.global::<Styling>();
    styling.on_preview({
        let (ed, win) = (editor.clone(), window.as_weak());
        move |field| {
            let Some(w) = win.upgrade() else { return };
            if field == StyleField::FontFamily {
                let s = w.global::<Styling>();
                let family = text::family_id(&s.get_font_family()).unwrap_or_else(text::default_family);
                push_family(&s, family);
            }
            let style = current_style(&w);
            let mut ed = ed.borrow_mut();
            let starting = !ed.restyling();
            let changed = ed.restyle_live(|shape, st| match field {
                StyleField::Stroke => st.color = style.color,
                StyleField::Fill if shape.fillable() => st.fill = style.fill,
                StyleField::Radius if shape.stroked() => st.radius = style.radius,
                _ if !matches!(shape, Shape::Text(_)) => {}
                // the weight moves with the family to one it has
                StyleField::FontFamily => {
                    st.font.family = style.font.family;
                    st.font.weight = style.font.weight;
                }
                StyleField::FontWeight => st.font.weight = style.font.weight,
                StyleField::FontSize => st.font.size = style.font.size,
                _ => {}
            });
            if !changed {
                return;
            }
            let c = w.global::<Canvas>();
            if starting {
                // the selection was just lifted off the main layer
                c.set_drag_offset(Vec2 { x: 0.0, y: 0.0 });
                c.set_drag_angle(0.0);
                c.set_overlay(slint::Image::from_rgba8(ed.layer.buf.clone()));
            }
            c.set_drag_overlay(slint::Image::from_rgba8(ed.drag_layer.buf.clone()));
            push_selection(&win, &ed); // a new width resizes the boxes
        }
    });
    styling.on_commit({
        let (ed, win) = (editor.clone(), window.as_weak());
        move || {
            let mut ed = ed.borrow_mut();
            if ed.end_restyle() {
                push_overlay(&win, &ed);
            }
        }
    });
    {
        let families: Vec<SharedString> = text::families().into_iter().map(Into::into).collect();
        styling.on_find_fonts(move |q| {
            let q = q.to_lowercase();
            let hits = families.iter().filter(|f| f.to_lowercase().contains(&q));
            ModelRc::new(VecModel::from(hits.cloned().collect::<Vec<_>>()))
        });
    }
    push_family(&styling, text::default_family());
    styling.on_to_hex(|c| {
        let mut hex = format!("#{:02x}{:02x}{:02x}", c.red(), c.green(), c.blue());
        if c.alpha() < 255 {
            hex += &format!("{:02x}", c.alpha());
        }
        hex.into()
    });
    styling.on_parse_hex(|s| match parse_hex(&s) {
        Some(color) => ParsedColor { ok: true, color },
        None => ParsedColor::default(),
    });

    let _ = window.run();

    Ok(())
}
