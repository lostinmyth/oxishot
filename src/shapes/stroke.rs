use slint::Rgba8Pixel;

pub struct Stroke {
    pub points: Vec<(f32, f32)>,
    pub color: Rgba8Pixel,
    pub radius: f32,
}
