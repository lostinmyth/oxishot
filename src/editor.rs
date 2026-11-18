use crate::{
    raster::Layer,
    shapes::{Point, Shape},
};

pub struct Editor {
    pub layer: Layer,
    pub history: Vec<Shape>,
    pub redo: Vec<Shape>,
    pub current: Option<Shape>,
    pub live_path: String,
}

impl Editor {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            layer: Layer::new(width, height),
            history: Vec::new(),
            redo: Vec::new(),
            current: None,
            live_path: String::new(),
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
        s.rasterize(&mut self.layer);
        self.history.push(s);
        self.redo.clear();
        self.live_path.clear();
        true
    }

    pub fn rebuild(&mut self) {
        self.layer.clear();
        for s in &self.history {
            s.rasterize(&mut self.layer);
        }
    }

    pub fn undo(&mut self) -> bool {
        let Some(s) = self.history.pop() else {
            return false;
        };
        self.redo.push(s);
        self.rebuild();
        true
    }

    pub fn redo(&mut self) -> bool {
        if let Some(shape) = self.redo.pop() {
            shape.rasterize(&mut self.layer);
            self.history.push(shape);
        }

        false
    }
}
