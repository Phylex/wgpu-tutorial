use std::sync::Arc;
use std::sync::Mutex;
use std::f32::consts::FRAC_PI_2;

// This import allows us to use the useful definitions from cgmath
// The e.g. define a function to construct the view transformation
// matrix
use cgmath::*;
use winit::{
    event::{DeviceEvent, ElementState, MouseScrollDelta, WindowEvent},
    keyboard::{PhysicalKey, KeyCode}
};
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

const SAFE_FRAC_PI_2: f32 = FRAC_PI_2 - 0.0001;

/// The ObserverControlls are the user interface to an observer it allows the user to
/// move the observer around and look at different objects in the scene/world
#[derive(Debug)]
pub struct CameraControlls {
    amount_left: f32,
    amount_right: f32,
    amount_forward: f32,
    amount_backward: f32,
    amount_up: f32,
    amount_down: f32,
    rotate_horizontal: f32,
    rotate_vertical: f32,
    scroll: f32,
    speed: f32,
    sensitivity: f32,
    mouse_pressed: bool,
}

impl CameraControlls {
    pub fn new(speed: f32, sensitivity: f32) -> Self {
        Self {
            amount_left: 0.0,
            amount_right: 0.0,
            amount_forward: 0.0,
            amount_backward: 0.0,
            amount_up: 0.0,
            amount_down: 0.0,
            rotate_horizontal: 0.0,
            rotate_vertical: 0.0,
            scroll: 0.0,
            speed,
            sensitivity,
            mouse_pressed: false,
        }
    }
    pub fn on_keyboard_input(&mut self, input: &winit::event::KeyEvent) -> bool {
        let amount: f32 = if input.state == ElementState::Pressed {
            1.0
        } else {
            0.0
        };
        match input.physical_key {
            PhysicalKey::Code(code) => {
                match code {
                    KeyCode::KeyR | KeyCode::ArrowUp => {
                        self.amount_forward = amount;
                        true
                    }
                    KeyCode::KeyH | KeyCode::ArrowDown => {
                        self.amount_backward = amount;
                        true
                    }
                    KeyCode::KeyS | KeyCode::ArrowLeft => {
                        self.amount_left = amount;
                        true
                    }
                    KeyCode::KeyT | KeyCode::ArrowRight => {
                        self.amount_right = amount;
                        true
                    }
                    KeyCode::Space => {
                        self.amount_up = amount;
                        true
                    }
                    KeyCode::ShiftLeft => {
                        self.amount_down = amount;
                        true
                    }
                    _ => false,
                }
            }
            PhysicalKey::Unidentified(_) => false,
        }
    }

    pub fn on_cursor_moved(&mut self, delta: &(f64, f64)) -> bool {
        if self.mouse_pressed {
            self.rotate_horizontal = delta.0 as f32;
            self.rotate_vertical = delta.1 as f32;
            true
        } else {
            false
        }
    }

    /// Process the mouse wheel input and indicate if it has been processed
    pub fn on_mouse_wheel(&mut self, delta: &winit::event::MouseScrollDelta) -> bool {
        self.scroll = match delta {
            MouseScrollDelta::LineDelta(_, scroll) => scroll * 100.0,
            MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition { y: scroll, .. }) => {
                *scroll as f32
            }
        };
        // currently all mouse wheel input is processed so we always return true
        true
    }

    /// Process the mouse button input. Currently we want to pan the camera if
    /// the left mouse button is pressed, we don't care about anything else for now
    /// also indicate if it has been processed
    pub fn on_mouse_button_input(
        &mut self,
        state: &winit::event::ElementState,
        button: &winit::event::MouseButton,
    ) -> bool {
        match button {
            winit::event::MouseButton::Left => {
                self.mouse_pressed = *state == winit::event::ElementState::Pressed;
                true
            }
            _ => false,
        }
    }

    pub fn on_device_event(&mut self, event: &DeviceEvent) -> bool {
        match event {
            DeviceEvent::MouseMotion { delta, .. } => self.on_cursor_moved(delta),
            _ => false,
        }
    }

    pub fn on_window_event(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput { event, .. } => self.on_keyboard_input(event),
            WindowEvent::MouseWheel { delta, .. } => self.on_mouse_wheel(delta),
            WindowEvent::MouseInput { state, button, .. } => {
                self.on_mouse_button_input(&state, &button)
            }
            _ => false,
        }
    }
}

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
    pub uniform: Arc<Mutex<CameraUniform>>,
    pub controls: CameraControlls,
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
        // the uniform is the thing that lives on the GPU
        // and which holds the final transform matrix of the
        // camera
        device: &wgpu::Device,

        // we need access to the command queue to write the transformation
        // matrix of this camera to the gpu memory
        queue: &wgpu::Queue,
    ) -> Self
    where
        V: Into<Point3<f32>>,
        P: Into<Rad<f32>>,
        Y: Into<Rad<f32>>,
        F: Into<Rad<f32>> + Copy,
    {
        let uniform = Arc::new(Mutex::new(CameraUniform::new(&device)));
        let cam = Camera {
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
            uniform,
            controls: CameraControlls::new(4.0, 0.4),
        };
        // the data in the GPU needs to actually be initialized, so we compute the matrix here and
        // then send it to the GPU
        {
            let mut uniform = cam.uniform.lock().unwrap();
            uniform.update(cam.compute_full_camera_transform(), queue);
        }
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

    pub fn resize(&mut self, screen_width: u32, screen_height: u32) {
        self.aspect_ratio = screen_width as f32 / screen_height as f32;
        self.set_perspective(self.field_of_view, self.aspect_ratio, self.znear, self.zfar)
    }

    /// Take the input of the controls and update the state of the camera transform matrix
    pub fn update(&mut self, dt: std::time::Duration) {
        let dt = dt.as_secs_f32();

        // process the moving around part of the camera
        let (yaw_sin, yaw_cos) = self.yaw.sin_cos();
        let (pitch_sin, pitch_cos) = self.pitch.sin_cos();
        let forward = Vector3::new(yaw_cos, pitch_sin, yaw_sin).normalize();
        let right = Vector3::new(-yaw_sin, 0.0, yaw_cos).normalize();
        self.position += forward * (self.controls.amount_forward - self.controls.amount_backward) * self.controls.speed * dt;
        self.position += right * (self.controls.amount_right - self.controls.amount_left) * self.controls.speed * dt;
        self.position += Vector3::unit_y() * (self.controls.amount_up - self.controls.amount_down) * self.controls.speed * dt;

        // process the scrolling motion and then reset it so that we don't scroll to
        // infinity
        let scrollward = Vector3::new(pitch_cos * yaw_cos, pitch_sin, pitch_cos * yaw_sin).normalize();
        self.position += scrollward * self.controls.scroll * self.controls.speed * self.controls.sensitivity * dt;
        self.controls.scroll = 0.;

        // update the view direction and then reset the control amount;
        self.yaw += Rad(self.controls.rotate_horizontal) * self.controls.sensitivity * dt;
        self.pitch += Rad(-self.controls.rotate_vertical) * self.controls.sensitivity * dt;
        self.controls.rotate_horizontal = 0.0;
        self.controls.rotate_vertical = 0.0;

        // limit the maximum and minimum pitch so we dont get gimball lock
        if self.pitch < -Rad(SAFE_FRAC_PI_2) {
            self.pitch = -Rad(SAFE_FRAC_PI_2);
        } else if self.pitch > Rad(SAFE_FRAC_PI_2) {
            self.pitch = Rad(SAFE_FRAC_PI_2);
        }
    }

    /// Compute the transform matrix that goes into the CameraUniform
    pub fn compute_full_camera_transform(&self) -> [[f32; 4]; 4] {
        (self.perspective * self.compute_view_matrix()).into()
    }

    
    pub fn update_uniform(&self, queue: &wgpu::Queue) {
        self.uniform.lock().unwrap().update(self.compute_full_camera_transform(), queue)
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
        // This hides complexity that would otherwise
        // be our responsibility. It essentially creates a 'staging buffer'
        // to which it writes the data and then adds a buffertobuffer copy operation to
        // the command queue
        queue.write_buffer(
            &self.gpu_buffer,
            0,
            bytemuck::cast_slice(&[camera_transform]),
        );
    }

    pub fn describe() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            label: Some("Camera bind group"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        }
    }

    // The following are helper functions to define the things that are needed on the GPU side for
    // everything to work

    /// Build the structure of the bind group from this function and regester it with the device
    fn create_gpu_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&CameraUniform::describe())
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
            // a buffer that is mapped at creation will be available as
            // a memory map on the CPU side to write into. This
            // means that
            mapped_at_creation: false,
        })
    }
}
