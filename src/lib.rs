// This import allows us to use the useful definitions from cgmath
// The e.g. define a function to construct the view transformation
// matrix
use cgmath::*;

// This is a transform between different reference frames. This is due to
// differing standard coordinate frames in openGL and WebGPU
#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
);

// This is the camera struct
#[derive(Debug)]
pub struct Camera {
    // This is the position of the camera in world space
    pub position: Point3<f32>,
    // The viewing direction is going to be defined by pitch
    // and yaw
    pub pitch: Rad<f32>,
    pub yaw: Rad<f32>,
    pub field_of_view: Rad<f32>,
    // this is the aspect ratio of our screen, which we need to generate
    // the view transformation matrix
    pub aspect_ratio: f32,
    // these are needed to determin clipping of objects to be rendered
    pub znear: f32,
    pub zfar: f32,
}
