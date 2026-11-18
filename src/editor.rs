use crate::{
    raster::Layer,
    shapes::{Point, Shape},
};

/// A reversible change to `Editor::shapes`. Applying one returns the op
/// that undoes it, so the undo and redo stacks just trade ops back and forth.
/// Index lists are sorted ascending.
pub enum Op {
    Insert(Vec<(usize, Shape)>),
    Remove(Vec<usize>),
    Move(Vec<usize>, Point),
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
    Move { from: Point },
    /// Rubber-band selection. `base` is the selection before it started and
    /// `bounds` caches every shape's bounds so moving the box stays cheap.
    Marquee {
        from: Point,
        to: Point,
        mode: SelectMode,
        base: Vec<usize>,
        bounds: Vec<Bounds>,
    },
}

/// Extra slack (px) around a shape's outline that still counts as a click on it.
const HIT_TOL: f32 = 4.0;
/// How far the UI draws a selection box outside the shape's bounds;
/// clicks anywhere inside that box hit the selected shape.
pub const SEL_PAD: f32 = 3.0;
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
        // selected shapes are hit anywhere inside their selection box;
        // everything else only on its painted outline
        let in_box = |i: usize| {
            let ((x0, y0), (x1, y1)) = self.shapes[i].bounds();
            p.0 >= x0 - SEL_PAD && p.0 <= x1 + SEL_PAD && p.1 >= y0 - SEL_PAD && p.1 <= y1 + SEL_PAD
        };
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
                bounds: self.shapes.iter().map(Shape::bounds).collect(),
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

        // lift the selection off the main layer onto its own
        self.gesture = Gesture::Move { from: p };
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
        true
    }

    /// Offset of the drag in progress, if any.
    pub fn drag_offset(&self, p: Point) -> Option<Point> {
        match self.gesture {
            Gesture::Move { from: (x, y) } => Some((p.0 - x, p.1 - y)),
            _ => None,
        }
    }

    pub fn dragging(&self) -> bool {
        matches!(self.gesture, Gesture::Move { .. })
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
            bounds,
        } = &mut self.gesture
        else {
            return false;
        };
        *to = p;
        let ((x0, y0), (x1, y1)) = span(*from, p);
        let tiny = x1 - x0 < MARQUEE_MIN && y1 - y0 < MARQUEE_MIN;
        // touching any part of a shape's selection box is enough
        let inside = |&((bx0, by0), (bx1, by1)): &Bounds| {
            !tiny
                && bx0 - SEL_PAD <= x1
                && bx1 + SEL_PAD >= x0
                && by0 - SEL_PAD <= y1
                && by1 + SEL_PAD >= y0
        };
        let mut base_it = base.iter().peekable();
        self.selected.clear();
        for (i, b) in bounds.iter().enumerate() {
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
            // drag_layer is left stale: the UI hides it, and press() clears it before reuse
            Gesture::Move { .. } => {
                match d {
                    Some(d) if d != (0.0, 0.0) => self.record(Op::Move(self.selected.clone(), d)),
                    // just a click: put the shapes back where they were
                    _ => self.rebuild(),
                }
                true
            }
        }
    }

    /// Bounds of each selected shape. Uses the marquee's cache while one is active.
    pub fn selection_bounds(&self) -> Vec<Bounds> {
        match &self.gesture {
            Gesture::Marquee { bounds, .. } => self.selected.iter().map(|&i| bounds[i]).collect(),
            _ => self.selected.iter().map(|&i| self.shapes[i].bounds()).collect(),
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

    pub fn select_all(&mut self) {
        self.selected = (0..self.shapes.len()).collect();
    }

    pub fn deselect(&mut self) -> bool {
        let had = !self.selected.is_empty();
        self.selected.clear();
        had
    }
}

/// Normalized rectangle spanned by two corners.
fn span(a: Point, b: Point) -> Bounds {
    ((a.0.min(b.0), a.1.min(b.1)), (a.0.max(b.0), a.1.max(b.1)))
}
