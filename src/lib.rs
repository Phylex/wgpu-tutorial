// This import allows us to use the useful definitions from cgmath
// The e.g. define a function to construct the view transformation
// matrix
use cgmath::*;

// This is a transform between different reference frames. This is due to
// differing standard coordinate frames in openGL and WebGPU. the cgmath
// package thinks in the openGL reference frame and wgpu in the WebGPU frame
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
    // The direction in which the lens is pointing
    // as angles relative to the world coordinate frame
    pub pitch: Rad<f32>,
    pub yaw: Rad<f32>,
    // field of view of the camera (something like the difference between
    // a zoom lense and a ultra wide lens)
    pub field_of_view: Rad<f32>,
    // this is the aspect ratio of our screen, which we need to generate
    // the view transformation matrix
    pub aspect_ratio: f32,
    // clipping distances that decide what distance things have to be to
    // to be rendered
    pub znear: f32,
    pub zfar: f32,

    // this is the perspective matrix. We only need to compute this very
    // seldomly so we store it instead of recomputing it each time we
    // update the GPU uniform
    perspective: Matrix4<f32>,
    uniform: CameraUniform,
}

// This is the struct that contains all the information to define
// the mathematical object that is the computer vision analogy of a
// camera (on a movie set for example).
impl Camera {
    pub fn new<V, Y, P, F>(
        position: V,
        pitch: P,
        yaw: Y,
        field_of_view: F,
        screen_width: u32,
        screen_height: u32,
        znear: f32,
        zfar: f32,
        // the camera is tied to it's representation on a GPU
        // so the device is the GPU it is tied to
        device: &wgpu::Device,
        // the update needs to be issued onto a command queue
        queue: &wgpu::Queue,
    ) -> Self
    where
        V: Into<Point3<f32>>,
        P: Into<Rad<f32>>,
        Y: Into<Rad<f32>>,
        F: Into<Rad<f32>> + Copy,
    {
        let mut cam = Camera {
            position: position.into(),
            pitch: pitch.into(),
            yaw: yaw.into(),
            field_of_view: field_of_view.clone().into(),
            aspect_ratio: screen_width as f32 / screen_height as f32,
            zfar,
            znear,
            perspective: Self::compute_projection_matrix(
                field_of_view,
                screen_width as f32 / screen_height as f32,
                zfar,
                znear,
            ),
            uniform: CameraUniform::new(device),
        };
        // the data in the GPU needs to actually be initialized, so we compute the matrix here and
        // then send it to the GPU
        cam.uniform
            .update((cam.perspective * cam.compute_view_matrix()).into(), queue);
        cam
    }
    // This is the matrix that distorts the world to emulate the 'lens' of the camera
    // When the result is projected onto a 2D plane it will look like a picture taken
    // with this virtual camera
    fn compute_projection_matrix<F>(fov: F, aspect: f32, znear: f32, zfar: f32) -> Matrix4<f32>
    where
        F: Into<Rad<f32>>,
    {
        OPENGL_TO_WGPU_MATRIX * perspective(fov.into(), aspect, znear, zfar)
    }

    // This is the matrix that moves all the vertices around such that it appears as
    // if we are looking at the world from the direction and position of our camera
    // we update this every time we move so
    fn compute_view_matrix(&self) -> Matrix4<f32> {
        // get the angles that we are looking at from the pitch and yaw
        // of the camera
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        // this `;ook to riht handed constructor builds the transform matrix
        // that let's us see the world from the point of view of the camera
        Matrix4::look_to_rh(
            self.position,
            // here we construct the vector, that points in the direction we
            // are pointing the camera
            Vector3::new(cos_pitch * cos_yaw, sin_pitch, cos_pitch * sin_yaw).normalize(),
            Vector3::unit_y(),
        )
    }

    // This is the matrix that distorts the world to emulate the 'lens' of the camera
    // When the result is projected onto a 2D plane it will look like a picture taken
    // with this virtual camera
    pub fn set_perspective<F>(&mut self, field_of_view: F, aspect_ratio: f32, znear: f32, zfar: f32)
    where
        F: Into<Rad<f32>> + Copy,
    {
        self.aspect_ratio = aspect_ratio;
        self.field_of_view = field_of_view.into();
        self.znear = znear;
        self.zfar = zfar;
        self.perspective =
            Self::compute_projection_matrix(field_of_view, aspect_ratio, znear, zfar);
    }
}

/// Struct that holds all data that is related to the representation of the Camera on the GPU
/// The camera will be a bind group that is accessible from the vertex shader so this is all set
/// up when this struct is instantiated.
#[derive(Debug)]
pub struct CameraUniform {
    gpu_buffer: wgpu::Buffer,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
}

impl CameraUniform {
    pub fn new(device: &wgpu::Device) -> Self {
        let gpu_buffer = Self::create_gpu_buffer(device);
        let bind_group_layout = Self::create_gpu_bind_group_layout(device);
        let bind_group = Self::create_bind_group(device, &bind_group_layout, &gpu_buffer);
        Self {
            gpu_buffer,
            bind_group_layout,
            bind_group,
        }
    }

    // when a new view transform is computed, this sends that new data to the buffer on the GPU
    pub fn update(&mut self, camera_transform: [[f32; 4]; 4], queue: &wgpu::Queue) {
        queue.write_buffer(
            &self.gpu_buffer,
            0,
            bytemuck::cast_slice(&[camera_transform]),
        );
    }
    // The following are helper functions to define the things that are needed on the GPU side for
    // everything to work

    /// Build the structure of the bind group from this function and regester it with the device
    fn create_gpu_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("observer bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        })
    }

    // actually create the bind group (the thing that is accessable from the shader) and put the
    // buffer containing the camera transformation into it
    fn create_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        proj_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            label: Some("Observer bind group"),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: proj_buffer.as_entire_binding(),
            }],
        })
    }

    // create the buffer for the camera uniform on the GPU
    fn create_gpu_buffer(device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Observer projection uniform buffer"),
            size: 16 * 4,
            // This buffer is the place that the view projection is placed in, so
            // we don't need the
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        })
    }
}
