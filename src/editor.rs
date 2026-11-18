use slint::Rgba8Pixel;

use crate::{
    raster::Layer,
    shapes::{Frame, Point, Scale, Shape, Style, points_bounds},
};

/// A reversible change to `Editor::shapes`. Applying one returns the op
/// that undoes it, so the undo and redo stacks just trade ops back and forth.
/// Index lists are sorted ascending.
pub enum Op {
    Insert(Vec<(usize, Shape)>),
    Remove(Vec<usize>),
    Move(Vec<usize>, Point),
    /// Rotate by an angle (radians) about a pivot.
    Rotate(Vec<usize>, Point, f32),
    Scale(Vec<usize>, Scale),
    /// Give each shape a new style.
    Restyle(Vec<(usize, Style)>),
}

/// How a select-tool press combines with the existing selection.
#[derive(Clone, Copy, PartialEq)]
pub enum SelectMode {
    Replace,
    Add,
    Remove,
}

pub type Bounds = (Point, Point);

enum Gesture {
    Idle,
    /// Dragging the selected shapes, which are lifted onto `drag_layer`.
    Move {
        from: Point,
    },
    /// Rotating the selected shapes about `pivot`, grabbed just outside a
    /// corner; they're lifted onto `drag_layer` just like a move. `start` is
    /// the press point's angle around the pivot. `frames` holds the selected
    /// shapes' frames at the start, so the boxes can turn without touching shapes.
    Rotate {
        pivot: Point,
        start: f32,
        angle: f32,
        frames: Vec<Frame>,
    },
    /// Scaling the selected shapes by a corner handle; they're lifted onto
    /// `drag_layer`, which is re-rasterized as the pointer moves (stroke
    /// widths stay put, so a stretched bitmap wouldn't match the result).
    /// `grab` is the grabbed corner in `scale.local` coordinates; `frames`
    /// holds the selected shapes' frames at the start.
    Scale {
        scale: Scale,
        grab: Point,
        uniform: bool,
        frames: Vec<Frame>,
    },
    /// Rubber-band selection. `base` is the selection before it started and
    /// `frames` caches every shape's frame so moving the box stays cheap.
    Marquee {
        from: Point,
        to: Point,
        mode: SelectMode,
        base: Vec<usize>,
        frames: Vec<Frame>,
    },
    /// Live-previewing a style change on the selection, which is lifted
    /// onto `drag_layer`. `originals` holds each selected shape's style before.
    Restyle {
        originals: Vec<(usize, Style)>,
    },
}

/// Extra slack (px) around a shape's outline that still counts as a click on it.
const HIT_TOL: f32 = 4.0;
/// How far the UI draws a selection box outside the shape's bounds;
/// clicks anywhere inside that box hit the selected shape.
pub const SEL_PAD: f32 = 3.0;
/// How far (px) outside a selection box's corner still grabs it for rotating.
const ROTATE_REACH: f32 = 14.0;
/// How close (px) to a selection box's corner grabs its scale handle.
/// Checked before rotating, so it wins where the two overlap.
const SCALE_REACH: f32 = 6.0;
/// Scaling never shrinks a selection box below this (px) along an axis.
const SCALE_MIN: f32 = 4.0;
/// A marquee smaller than this (px, both axes) is treated as a click and selects nothing.
const MARQUEE_MIN: f32 = 3.0;

pub struct Editor {
    /// Rasterized shapes. While dragging, the dragged shapes are left out.
    pub layer: Layer,
    /// Only the shapes being dragged; the UI offsets it instead of re-rasterizing.
    pub drag_layer: Layer,
    pub shapes: Vec<Shape>,
    undo: Vec<Op>,
    redo: Vec<Op>,
    pub current: Option<Shape>,
    pub live_path: String,
    /// Indices into `shapes`, sorted ascending.
    pub selected: Vec<usize>,
    gesture: Gesture,
}

impl Editor {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            layer: Layer::new(width, height),
            drag_layer: Layer::new(width, height),
            shapes: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            current: None,
            live_path: String::new(),
            selected: Vec::new(),
            gesture: Gesture::Idle,
        }
    }

    pub fn begin(&mut self, shape: Shape) {
        shape.write_preview(&mut self.live_path);
        self.current = Some(shape);
    }

    pub fn extend(&mut self, p: Point) -> bool {
        match &mut self.current {
            Some(s) => s.extend(p, &mut self.live_path),
            None => false,
        }
    }
    pub fn end(&mut self) -> bool {
        let Some(s) = self.current.take() else {
            return false;
        };
        self.commit(s);
        self.live_path.clear();
        true
    }

    /// Adds a finished shape to the layer and history.
    pub fn commit(&mut self, s: Shape) {
        let op = Op::Insert(vec![(self.shapes.len(), s)]);
        self.record(op);
    }

    fn rebuild(&mut self) {
        self.layer.clear();
        for s in &self.shapes {
            s.rasterize(&mut self.layer);
        }
    }

    /// Applies `op` and returns its inverse.
    fn apply(&mut self, op: Op) -> Op {
        match op {
            Op::Insert(items) => {
                // a single shape on top has nothing above it, so painting it
                // over the layer is exact
                let on_top = matches!(&items[..], [(i, _)] if *i == self.shapes.len());
                let idx = items.iter().map(|(i, _)| *i).collect();
                // ascending order puts each shape back at its original index
                for (i, s) in items {
                    self.shapes.insert(i, s);
                }
                if on_top {
                    self.shapes.last().unwrap().rasterize(&mut self.layer);
                } else {
                    self.rebuild();
                }
                Op::Remove(idx)
            }
            Op::Remove(idx) => {
                // descending so earlier indices stay valid
                let mut items: Vec<_> = idx
                    .iter()
                    .rev()
                    .map(|&i| (i, self.shapes.remove(i)))
                    .collect();
                items.reverse();
                self.rebuild();
                Op::Insert(items)
            }
            Op::Move(idx, (dx, dy)) => {
                for &i in &idx {
                    self.shapes[i].translate((dx, dy));
                }
                self.rebuild();
                Op::Move(idx, (-dx, -dy))
            }
            Op::Rotate(idx, pivot, angle) => {
                for &i in &idx {
                    self.shapes[i].rotate(pivot, angle);
                }
                self.rebuild();
                Op::Rotate(idx, pivot, -angle)
            }
            Op::Scale(idx, s) => {
                for &i in &idx {
                    self.shapes[i].scale(&s);
                }
                self.rebuild();
                Op::Scale(idx, s.inverse())
            }
            Op::Restyle(mut items) => {
                for (i, style) in &mut items {
                    std::mem::swap(self.shapes[*i].style_mut(), style);
                }
                self.rebuild();
                Op::Restyle(items)
            }
        }
    }

    /// Applies a new user edit, making it undoable.
    fn record(&mut self, op: Op) {
        let inv = self.apply(op);
        self.undo.push(inv);
        self.redo.clear();
    }

    pub fn undo(&mut self) -> bool {
        let Some(op) = self.undo.pop() else {
            return false;
        };
        // indices may shift under the selection
        self.selected.clear();
        let inv = self.apply(op);
        self.redo.push(inv);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(op) = self.redo.pop() else {
            return false;
        };
        self.selected.clear();
        let inv = self.apply(op);
        self.undo.push(inv);
        true
    }

    /// Select-tool press at `p`. Returns whether the layers changed
    /// (a drag started, so shapes were lifted onto `drag_layer`).
    pub fn press(&mut self, p: Point, mode: SelectMode) -> bool {
        if let Some((j, k)) = self.scale_zone(p) {
            let frames = self.selection_frames();
            let f = frames[j];
            let c = f.corners(0.0);
            // the opposite corner stays put
            let scale = Scale {
                anchor: c[(k + 2) % 4],
                angle: f.angle,
                factors: (1.0, 1.0),
            };
            // shapes turned differently can only share a uniform scale, and
            // text can't stretch; shift (the add modifier) keeps the aspect too
            let uniform = mode == SelectMode::Add
                || frames.len() > 1
                || matches!(self.shapes[self.selected[j]], Shape::Text(_));
            self.gesture = Gesture::Scale {
                grab: scale.local(c[k]),
                scale,
                uniform,
                frames,
            };
            self.lift();
            return true;
        }
        if self.rotate_zone(p).is_some() {
            let frames = self.selection_frames();
            let ((x0, y0), (x1, y1)) =
                points_bounds(frames.iter().flat_map(|f| f.corners(0.0)), 0.0);
            let pivot = ((x0 + x1) * 0.5, (y0 + y1) * 0.5);
            let start = (p.1 - pivot.1).atan2(p.0 - pivot.0);
            self.gesture = Gesture::Rotate {
                pivot,
                start,
                angle: 0.0,
                frames,
            };
            self.lift();
            return true;
        }

        // selected shapes are hit anywhere inside their selection box;
        // everything else only on its painted outline
        let in_box = |i: usize| self.shapes[i].frame().contains(p, SEL_PAD);
        let hit = (self.selected.iter().rev().copied().find(|&i| in_box(i)))
            .or_else(|| self.shapes.iter().rposition(|s| s.hit(p, HIT_TOL)));
        let Some(i) = hit else {
            if mode == SelectMode::Replace {
                self.selected.clear();
            }
            self.gesture = Gesture::Marquee {
                from: p,
                to: p,
                mode,
                base: self.selected.clone(),
                frames: self.shapes.iter().map(Shape::frame).collect(),
            };
            return false;
        };

        let pos = self.selected.binary_search(&i);
        match (mode, pos) {
            (SelectMode::Remove, Ok(k)) => {
                self.selected.remove(k);
                return false;
            }
            (SelectMode::Remove, Err(_)) => return false,
            // clicking an already-selected shape keeps the group, so it can be dragged
            (_, Ok(_)) => {}
            (SelectMode::Add, Err(k)) => self.selected.insert(k, i),
            (SelectMode::Replace, Err(_)) => self.selected = vec![i],
        }

        self.gesture = Gesture::Move { from: p };
        self.lift();
        true
    }

    /// Moves the selection off the main layer onto `drag_layer`.
    fn lift(&mut self) {
        self.layer.clear();
        self.drag_layer.clear();
        let mut sel = self.selected.iter().peekable();
        for (j, s) in self.shapes.iter().enumerate() {
            if sel.next_if_eq(&&j).is_some() {
                s.rasterize(&mut self.drag_layer);
            } else {
                s.rasterize(&mut self.layer);
            }
        }
    }

    /// (selection index, corner index) of the scale handle under `p`.
    fn scale_zone(&self, p: Point) -> Option<(usize, usize)> {
        if !matches!(self.gesture, Gesture::Idle) {
            return None;
        }
        self.selection_frames()
            .iter()
            .enumerate()
            .find_map(|(j, f)| {
                let k = (f.corners(SEL_PAD).iter()).position(|&c| dist(p, c) <= SCALE_REACH)?;
                Some((j, k))
            })
    }

    /// If `p` is on a selected shape's corner handle, where a press starts
    /// scaling, returns which way that corner faces: radians clockwise from
    /// the top-left corner.
    pub fn scale_handle(&self, p: Point) -> Option<f32> {
        let (j, k) = self.scale_zone(p)?;
        Some(self.selection_frames()[j].angle + k as f32 * std::f32::consts::FRAC_PI_2)
    }

    /// If `p` is just outside a corner of a selected shape's box, where a
    /// press starts rotating the selection about its center, returns which
    /// way that corner faces: radians clockwise from the top-left corner.
    pub fn rotate_zone(&self, p: Point) -> Option<f32> {
        if !matches!(self.gesture, Gesture::Idle) || self.scale_zone(p).is_some() {
            return None;
        }
        self.selection_frames().iter().find_map(|f| {
            if f.contains(p, SEL_PAD) {
                return None;
            }
            // corners() goes clockwise from top-left, a quarter turn apart
            let k = (f.corners(SEL_PAD).iter()).position(|&c| dist(p, c) <= ROTATE_REACH)?;
            Some(f.angle + k as f32 * std::f32::consts::FRAC_PI_2)
        })
    }

    /// Updates an active rotation for the pointer at `p`.
    /// Returns (pivot, angle in radians), or None when not rotating.
    pub fn rotate_to(&mut self, p: Point) -> Option<(Point, f32)> {
        let Gesture::Rotate {
            pivot,
            start,
            angle,
            ..
        } = &mut self.gesture
        else {
            return None;
        };
        *angle = (p.1 - pivot.1).atan2(p.0 - pivot.0) - *start;
        Some((*pivot, *angle))
    }

    pub fn rotating(&self) -> bool {
        matches!(self.gesture, Gesture::Rotate { .. })
    }

    /// Updates an active scale for the pointer at `p` and redraws
    /// `drag_layer`. Returns false when not scaling.
    pub fn scale_to(&mut self, p: Point) -> bool {
        let Gesture::Scale {
            scale,
            grab: g,
            uniform,
            ..
        } = &mut self.gesture
        else {
            return false;
        };
        let v = scale.local(p);
        scale.factors = if *uniform {
            // project the pointer onto the anchor-to-corner diagonal
            let g2 = g.0 * g.0 + g.1 * g.1;
            let s = if g2 < 1e-6 {
                1.0
            } else {
                ((v.0 * g.0 + v.1 * g.1) / g2).max(SCALE_MIN / g2.sqrt())
            };
            (s, s)
        } else {
            (axis_factor(v.0, g.0), axis_factor(v.1, g.1))
        };
        let s = *scale;

        // a fresh buffer instead of clear(): the UI still holds the old one,
        // so clearing would first copy it just to overwrite it
        self.drag_layer = Layer::new(self.drag_layer.buf.width(), self.drag_layer.buf.height());
        for &i in &self.selected {
            let mut shape = self.shapes[i].clone();
            shape.scale(&s);
            shape.rasterize(&mut self.drag_layer);
        }
        true
    }

    pub fn scaling(&self) -> bool {
        matches!(self.gesture, Gesture::Scale { .. })
    }

    /// Offset of the drag in progress, if any.
    pub fn drag_offset(&self, p: Point) -> Option<Point> {
        match self.gesture {
            Gesture::Move { from: (x, y) } => Some((p.0 - x, p.1 - y)),
            _ => None,
        }
    }

    /// Whether the selection is lifted onto `drag_layer` (moving, rotating or scaling).
    pub fn dragging(&self) -> bool {
        matches!(
            self.gesture,
            Gesture::Move { .. }
                | Gesture::Rotate { .. }
                | Gesture::Scale { .. }
                | Gesture::Restyle { .. }
        )
    }

    /// The marquee rectangle, if one is being drawn.
    pub fn marquee(&self) -> Option<Bounds> {
        match self.gesture {
            Gesture::Marquee { from, to, .. } => Some(span(from, to)),
            _ => None,
        }
    }

    /// Moves the marquee's free corner to `p`, updating the selection live.
    /// Returns false when no marquee is active.
    pub fn marquee_to(&mut self, p: Point) -> bool {
        let Gesture::Marquee {
            from,
            to,
            mode,
            base,
            frames,
        } = &mut self.gesture
        else {
            return false;
        };
        *to = p;
        let ((x0, y0), (x1, y1)) = span(*from, p);
        let tiny = x1 - x0 < MARQUEE_MIN && y1 - y0 < MARQUEE_MIN;
        // touching any part of a shape's selection box is enough
        let inside = |f: &Frame| {
            let ((bx0, by0), (bx1, by1)) = f.aabb();
            !tiny
                && bx0 - SEL_PAD <= x1
                && bx1 + SEL_PAD >= x0
                && by0 - SEL_PAD <= y1
                && by1 + SEL_PAD >= y0
        };
        let mut base_it = base.iter().peekable();
        self.selected.clear();
        for (i, b) in frames.iter().enumerate() {
            let was = base_it.next_if_eq(&&i).is_some();
            let keep = match mode {
                SelectMode::Remove => was && !inside(b),
                _ => was || inside(b),
            };
            if keep {
                self.selected.push(i);
            }
        }
        true
    }

    /// Ends the select gesture at `p`. Returns whether the layers changed.
    pub fn release(&mut self, p: Point) -> bool {
        let d = self.drag_offset(p);
        match std::mem::replace(&mut self.gesture, Gesture::Idle) {
            Gesture::Idle | Gesture::Marquee { .. } => false,
            // not a pointer gesture; put it back for end_restyle
            g @ Gesture::Restyle { .. } => {
                self.gesture = g;
                false
            }
            // drag_layer is left stale: the UI hides it, and press() clears it before reuse
            Gesture::Move { .. } => {
                match d {
                    Some(d) if d != (0.0, 0.0) => self.record(Op::Move(self.selected.clone(), d)),
                    // just a click: put the shapes back where they were
                    _ => self.rebuild(),
                }
                true
            }
            Gesture::Rotate { pivot, angle, .. } => {
                if angle != 0.0 {
                    self.record(Op::Rotate(self.selected.clone(), pivot, angle));
                } else {
                    self.rebuild();
                }
                true
            }
            Gesture::Scale { scale, .. } => {
                if scale.factors != (1.0, 1.0) {
                    self.record(Op::Scale(self.selected.clone(), scale));
                } else {
                    self.rebuild();
                }
                true
            }
        }
    }

    /// Frame of each selected shape. Uses the gesture's cache while a
    /// marquee, rotation or scale is active (transformed by it so far).
    pub fn selection_frames(&self) -> Vec<Frame> {
        match &self.gesture {
            Gesture::Marquee { frames, .. } => self.selected.iter().map(|&i| frames[i]).collect(),
            Gesture::Rotate {
                pivot,
                angle,
                frames,
                ..
            } => frames
                .iter()
                .map(|f| f.rotated_about(*pivot, *angle))
                .collect(),
            Gesture::Scale { scale, frames, .. } => {
                frames.iter().map(|f| f.scaled(scale)).collect()
            }
            _ => self
                .selected
                .iter()
                .map(|&i| self.shapes[i].frame())
                .collect(),
        }
    }

    pub fn delete_selected(&mut self) -> bool {
        if self.selected.is_empty() {
            return false;
        }
        let idx = std::mem::take(&mut self.selected);
        self.record(Op::Remove(idx));
        true
    }

    /// Applies `f` to each selected shape's style as a live preview: the
    /// selection is lifted onto `drag_layer` (on the first call) and only that
    /// is redrawn, so dragging through a color picker stays cheap.
    /// `end_restyle` records it all as one edit. Returns false when nothing
    /// is selected or another gesture is running.
    pub fn restyle_live(&mut self, f: impl Fn(&Shape, &mut Style)) -> bool {
        if self.selected.is_empty() {
            return false;
        }
        match self.gesture {
            Gesture::Restyle { .. } => {}
            Gesture::Idle => {
                let originals = (self.selected.iter())
                    .map(|&i| (i, *self.shapes[i].style()))
                    .collect();
                self.gesture = Gesture::Restyle { originals };
                self.lift();
            }
            _ => return false,
        }
        // a fresh buffer instead of clear(), as in scale_to
        self.drag_layer = Layer::new(self.drag_layer.buf.width(), self.drag_layer.buf.height());
        for &i in &self.selected {
            let shape = &mut self.shapes[i];
            let mut style = *shape.style();
            f(shape, &mut style);
            *shape.style_mut() = style;
            shape.rasterize(&mut self.drag_layer);
        }
        true
    }

    pub fn restyling(&self) -> bool {
        matches!(self.gesture, Gesture::Restyle { .. })
    }

    /// Ends a live restyle, making it one undoable edit (if anything
    /// changed). Returns whether the layers changed.
    pub fn end_restyle(&mut self) -> bool {
        if !self.restyling() {
            return false;
        }
        let Gesture::Restyle { originals } = std::mem::replace(&mut self.gesture, Gesture::Idle)
        else {
            unreachable!()
        };
        // the shapes already hold the new styles, so the originals are the inverse
        let inv: Vec<_> = (originals.into_iter())
            .filter(|(i, st)| self.shapes[*i].style() != st)
            .collect();
        if !inv.is_empty() {
            self.undo.push(Op::Restyle(inv));
            self.redo.clear();
        }
        self.rebuild();
        true
    }

    /// Distinct colors the shapes paint with (strokes and fills), most
    /// recently drawn first, at most `max`.
    pub fn used_colors(&self, max: usize) -> Vec<Rgba8Pixel> {
        let mut out = Vec::new();
        let styles = self.shapes.iter().rev().map(|s| (s.style(), s.fillable()));
        for (st, fillable) in styles {
            let fill = st.fill.filter(|_| fillable);
            for c in std::iter::once(st.color).chain(fill) {
                if c.a > 0 && !out.contains(&c) {
                    out.push(c);
                    if out.len() == max {
                        return out;
                    }
                }
            }
        }
        out
    }

    pub fn select_all(&mut self) {
        self.selected = (0..self.shapes.len()).collect();
    }

    pub fn deselect(&mut self) -> bool {
        let had = !self.selected.is_empty();
        self.selected.clear();
        had
    }
}

/// Scale factor along one axis that brings the grabbed corner (`g` from the
/// anchor) to the pointer (`v`), never flipping or shrinking below `SCALE_MIN`.
fn axis_factor(v: f32, g: f32) -> f32 {
    if g.abs() < 1e-3 {
        return 1.0;
    }
    (v / g).max(SCALE_MIN / g.abs())
}

fn dist(a: Point, b: Point) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// Normalized rectangle spanned by two corners.
fn span(a: Point, b: Point) -> Bounds {
    ((a.0.min(b.0), a.1.min(b.1)), (a.0.max(b.0), a.1.max(b.1)))
}
