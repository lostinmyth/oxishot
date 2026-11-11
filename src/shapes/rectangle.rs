use slint::Rgba8Pixel;

pub struct Rectangle {
    pub a: (f32, f32), // corner where the drag started
    pub b: (f32, f32), // current/final corner
    pub color: Rgba8Pixel,
    pub radius: f32, // half the line width, same meaning as Stroke.radius
}
