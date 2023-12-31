use std::{time::{Instant, Duration}, sync::{Mutex, Arc}, ops::Deref};
use std::iter;

use camera::CameraUniform;
use cgmath;
use colored_mesh_renderer::ColoredMeshRenderer;
use model::DrawMesh;
use renderer::DescribeRenderPipeline;
use wgpu::util::DeviceExt;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{WindowBuilder, Window}, dpi::PhysicalSize,
};
use egui_winit;
use egui_wgpu;
use egui;

mod camera;
mod model;
mod renderer;
mod instance;
mod colored_mesh_renderer;
mod resources;

// We need a place to put the objects/data related to the global state into
struct App {
    window: Window, // The winit Window
    // we need to keep the size here so that we can detect when the screen size
    // changes
    window_size: PhysicalSize<u32>,
    // the connection of the gpu with the window so that the GPU can draw stuff
    surface_config: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface, 
    // The representation of the GPU driver in our application
    instance: wgpu::Instance,
    // The physical card used for precessing the rendering commands
    // and storing the objects sent to the GPU
    adapter: wgpu::Adapter,
    // the logical connection to the GPU
    device: wgpu::Device,
    // The command queue that provides the context for the rendering commands
    // we first create a sequence of commands and then have the GPU driver
    // execute those when we submit the queue to the GPU
    queue: wgpu::Queue,

    // our render pipeline
    render_pipeline: ColoredMeshRenderer,

    //camera structs 
    cameras: Vec<camera::Camera>,
    // uniform
    camera_uniform: Arc<Mutex<camera::CameraUniform>>,

    // active camera
    active_camera: usize,

    // the depth texture for the render to the screen
    depth_texture: model::Texture,
    
    // This is where we store the objects that we want to render
    objects: Vec<model::Object>,

    // this is all the egui stuff we need to have a UI visible
    ui_context: egui::Context,
    ui_painter: egui_wgpu::renderer::Renderer,
    ui_state: egui_winit::State,
    ui_screen_descriptor: egui_wgpu::renderer::ScreenDescriptor,
    instance_buffer: wgpu::Buffer,
    model_instance: instance::Instance,
}

impl App {
    async fn new(window: Window) -> Self {
        let window_size = window.inner_size();
        
        // Now that an event loop and a window have been generated/procured from the os
        // we procede to initialize the GPU driver/WGPU
        // First off is the instance, this is the object that represents the environment
        // on the current machine
        // The pattern of 'descriptor structure' and create call is a typical pattern in
        // Vulkan
        let instance_descriptor = wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: wgpu::Dx12Compiler::Fxc,
            flags: wgpu::InstanceFlags::all(),
            gles_minor_version: wgpu::Gles3MinorVersion::Automatic,
        };
        let instance = wgpu::Instance::new(instance_descriptor);

        // this is the thing we use to render onto. It is created using
        // the window handle we get from winit
        // This is unsafe in the sense that we need to guarantee that the window
        // lives at least as long as the surface
        let surface = unsafe { instance.create_surface(&window).unwrap() };

        // A single Instance can manage multiple physical adapters (cards)
        // so now we need to describe and then request the graphics card we actually
        // want.
        let adapter_descriptor = wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            // here we pass the surface to the adapter so it can render to it
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        };
        // wait for the gpu driver to set up everything so that we can talk to the GPU
        let adapter = instance.request_adapter(
            &adapter_descriptor)
            .await.unwrap();
        // A single physical card can be split into many logical devices. A device is
        // the thing that performs the work while the queue is where the CPU prepares
        // the commands in the order in which they shoud be executed. We could have
        // multiple devices and queues in a single application that even run on the same
        // physical card but represent different threads of operations, that are indipen
        // dent of each other
        let device_descriptor = wgpu::DeviceDescriptor {
            label: Some("Main Device"), // we don't give this logical thread a name
            features: wgpu::Features::POLYGON_MODE_LINE | wgpu::Features::POLYGON_MODE_POINT, // we
            // need the line mode to draw the wireframes
            limits: wgpu::Limits::default(),
        };
        let (device, queue) = adapter.request_device(&device_descriptor, None).await.unwrap();
        
        // now we that we have the window and the rendering device we
        // need to configure the surface so that we can render to it properly
        // first off, we find out what operations this surface actually supports
        //
        let surface_capabilities = surface.get_capabilities(&adapter);
        // we want a surface with a srgb format, otherwise we panic
        let surface_format = surface_capabilities.formats.iter()
            .copied()
            .filter(|f| f.is_srgb())
            .next()
            .unwrap(); 
        // we now set up the surface configuration that we want and then configure
        // the surface
        // The surface becomes a texture (in the context of wgpu). It is given to a
        // render pass as color attachement which is where the GPU ends up rendering
        // things to.
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: window_size.width,
            height: window_size.height,
            present_mode: surface_capabilities.present_modes[0],
            alpha_mode: surface_capabilities.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        // now that we have all the necessary resources to draw to a screen, we can start
        // to construct various render pipelines and then use those to render stuff to
        // the screen (or a texture) in a render pass.
        // 
        // The main component of a render pipeline are the two shaders. The shaders define
        // how the render pipeline needs to be layed out. They also define the interpretation
        // of the data. So as a result, pipelines are really just hand crafted.
        //
        // This concludes the setup of the device. At this stage we have set up the
        // environment and we start creating the objects that we want to draw on the
        // screen this is 
        //
        // The thing that manages the context for a render is the RenderPass. It is
        // a set of commands that is 'recorded' on the application side and then
        // submitted
        // to the GPU for processing
        //
        // First of all, we need to put the data that we want to render onto the GPU.
        // with `data` I mean both shader code as well as textures and 'uniform' data and
        // last but not least the vertex data that is used to describe the geometry in 
        // 3dimensions.
        //
        // The thing that I'd like to acheive is to be able to manipulate 3D mesh data that forms
        // all kinds of different things and have it rendered. This means that this is simply the
        // rendering part of the app that needs to contain a modeling part as well.
        
        // so we instaltiate a camera, the camera does not include the buffer in the GPU, that is
        // the CameraUniform which is separate. We can however write the content to the Camera
        // Uniform, this allows us to have multiple cameras, but only one buffer on the GPU.
        let camera_uniform = Arc::new(Mutex::new(CameraUniform::new(&device)));
        let camera = camera::Camera::new(
            (1.0, 0.0, 0.0),
            cgmath::Deg(-20.0),
            cgmath::Deg(-90.0),
            cgmath::Deg(45.0),
            window_size.width,
            window_size.height,
            0.1,
            100.0,
            camera_uniform.clone(),
            &queue
        );

        // this texture holds the depth information that is used for the z-buffer algorithm.
        let depth_texture = model::Texture::create_depth_texture(&device, &config, "depth texture");

        // now we create the render pipeline and the pipeline controller, the pipeline controller
        // won't be important right now, but we will use it when we have more than one pipeline.
        let color_render_pipeline = colored_mesh_renderer::ColoredMeshRenderer::new(
            &device,
            &camera.uniform.lock().as_ref().unwrap().bind_group_layout,
            &config,
            Some(model::Texture::DEPTH_FORMAT),
        );

        // now that we have set up our own pipeline, we need to set up the pipeline that draws to
        // to the ui to the screen this is somewhat important as we need the UI to do control the
        // rendering
        let ui_context = egui::Context::default();
        let ui_state = egui_winit::State::new(
            ui_context.viewport_id(),
            &window,
            Some(window.scale_factor() as f32),
            None
        );
        let ui_renderer = egui_wgpu::renderer::Renderer::new(&device, surface_format, Some(model::Texture::DEPTH_FORMAT), 1);
        let ui_screen_descriptor = egui_wgpu::renderer::ScreenDescriptor{ size_in_pixels: [config.width, config.height], pixels_per_point: 2. };

        let initial_object = resources::load_model("teapot.obj", &device, &queue).await.unwrap();

        let model_instance = instance::Instance {
            position: [0., 0., 0.].into(), 
            rotation: cgmath::Quaternion::from_sv(0.0, cgmath::Vector3::unit_z()),
            scale: [1.0, 1.0, 1.0].into(),
            color: [1., 0., 1., 1.].into()};
        let instance_data = model_instance.compute_instance_matrix();
        let instance_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Instance Buffer"),
                contents: bytemuck::cast_slice(&instance_data),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            }
        );
            
        let global_camera = camera.uniform.clone();
        App {
            window,
            window_size,
            surface,
            instance,
            adapter,
            device,
            queue,
            depth_texture,
            render_pipeline: color_render_pipeline,
            cameras: vec![camera],
            camera_uniform,
            objects: vec![initial_object],
            ui_context,
            ui_painter: ui_renderer,
            ui_screen_descriptor,
            ui_state,
            active_camera: 0,
            surface_config: config,
            model_instance,
            instance_buffer,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            println!("Resize occurred: width {}, height {}", new_size.width, new_size.height);
            self.surface_config.width  = new_size.width;
            self.surface_config.height = new_size.height;
            self.surface.configure(&self.device, &self.surface_config);
            for camera in self.cameras.iter_mut() {
                camera.resize(new_size.width, new_size.height);
            }
            self.depth_texture = model::Texture::create_depth_texture(&self.device, &self.surface_config, "depth texture");
            self.ui_screen_descriptor = egui_wgpu::renderer::ScreenDescriptor{ size_in_pixels: [new_size.width, new_size.height], pixels_per_point: 2. };
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        // This is the texture we are going to render the output to. We get the texture from the
        // surface meaning it will be a texture that is part of the swapchain.
        let output = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Outdated) => {
                // This error occurs when the app is minimized on Windows.
                // This also means that we don't need to render anything so we
                // can simply return from the render function without actually rendering
                // anything
                //
                // Silently return here to prevent spamming the console with:
                // "The underlying surface has changed, and therefore the swap chain must be updated"
                return Ok(());
            }
            Err(e) => {
                eprintln!("Dropped frame with error: {}", e);
                return Err(e);
            }
        };
        // every texture needs a texture view to be accessible to the render pipeline, so we create
        // a default one.
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // this collects all the operations we want the GPU to perform. It is sent as a batch to
        // the GPU to be processed
        let depth_texture_view = &mut self.depth_texture.view;
        let camera_uniform = self.camera_uniform.lock().unwrap();
        let color_attachment = [ColoredMeshRenderer::describe_color_attachment(Some(&view))];
        let depth_stencil_attachment = ColoredMeshRenderer::describe_depth_stencil(Some(depth_texture_view));

        // process the ui specific things before starting with the render pass
        let ui_input = self.ui_state.take_egui_input(&self.window);
        let ui_output = self.ui_context.run(ui_input, |ctx| {
            egui::Window::new("Color Controls").show(&ctx, |ui| {
                ui.label("Hello world!");
                if ui.button("Change Color").clicked() {
                    if self.model_instance.color.x == 1. {
                        self.model_instance.color.x = 0.;
                    } else {
                        self.model_instance.color.x = 1.;
                    }
                }
                self.queue.write_buffer(&self.instance_buffer, 0, &bytemuck::cast_slice(&self.model_instance.compute_instance_matrix()));
            });
        });
        self.ui_state.handle_platform_output(&self.window, &self.ui_context, ui_output.platform_output);
        let ui_primitives = self.ui_context.tessellate(ui_output.shapes, ui_output.pixels_per_point);

        // prepare all the buffers and such
        for (id, image_delta) in &ui_output.textures_delta.set {
            self.ui_painter.update_texture(&self.device, &self.queue, *id, &image_delta);
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Main render encoder"),
            });
        self.ui_painter.update_buffers(&self.device, &self.queue, &mut encoder, &ui_primitives, &self.ui_screen_descriptor);
        {
            let mut render_pass = encoder.begin_render_pass(&ColoredMeshRenderer::describe_render_pass(&color_attachment, depth_stencil_attachment));
            render_pass.set_pipeline(&self.render_pipeline.pipeline);
            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            for obj in self.objects.iter() {
                for mesh in obj.meshes.iter() {
                    ColoredMeshRenderer::draw_mesh(&mut render_pass, mesh, &camera_uniform.deref().bind_group);
                }
            }
            self.ui_painter.render(&mut render_pass, &ui_primitives, &self.ui_screen_descriptor);
        }
        for id in &ui_output.textures_delta.free {
            self.ui_painter.free_texture(id);
        }
        self.queue.submit(iter::once(encoder.finish()));
        output.present();
        Ok(())
    }

    pub fn update(&mut self, dt: Duration) {
        self.cameras[self.active_camera].update(dt);
        self.cameras[self.active_camera].update_uniform(&self.queue);
    }
    
    fn on_event(&mut self, event: &Event<()>, control_flow: &mut ControlFlow, last_render_time: &mut Instant) {
        match event {
            Event::WindowEvent { window_id, event, .. } if *window_id == self.window.id() => {
                // let the ui handle the input
                let resp = self.ui_state.on_window_event(&self.ui_context, event);
                // pass the input to the camera for it to process stuff
                let processed = if !resp.consumed {
                    self.cameras[self.active_camera].controls.on_window_event(event)
                } else {
                    false
                };
                if !(resp.consumed && processed) {
                    match event {
                        WindowEvent::CloseRequested
                        | WindowEvent::KeyboardInput {
                            input:
                                KeyboardInput {
                                    state: ElementState::Pressed,
                                    virtual_keycode: Some(VirtualKeyCode::Escape),
                                    ..
                                },
                            ..
                        } => *control_flow = ControlFlow::Exit,
                        WindowEvent::Resized(physical_size) => {
                            self.resize(*physical_size);
                        }
                        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                            // new_inner_size is &mut so w have to dereference it twice
                            self.resize(**new_inner_size);
                        }
                        _ => {}
                    }
                }
            },
            Event::DeviceEvent { event, .. } => {
                _ = self.cameras[self.active_camera].controls.on_device_event(&event);
            },
            Event::RedrawRequested(window_id) if *window_id == self.window.id() => {
                let now = Instant::now();
                let dt = now - last_render_time.clone();
                *last_render_time = now;
                self.update(dt);
                match self.render() {
                    Ok(_) => {}
                    // Reconfigure the surface if it's lost or outdated
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => self.resize(self.window_size),
                    // The system is out of memory, we should probably quit
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    // We're ignoring timeouts
                    Err(wgpu::SurfaceError::Timeout) => log::warn!("Surface timeout"),
                }
            }
            Event::MainEventsCleared => {
                // RedrawRequested will only trigger once, unless we manually
                // request it.
                self.window.request_redraw();
            }
            _ => {}
        }
    }
}

async fn run() {
    // This sets up a logger so that we can track what we are doing
    env_logger::init();
    
    // first of all we create the event loop that gathers the events
    // like button presses and mouse movements/clicks from the window,
    // as well as provide us with a mechanism to draw the our output on
    // the screen
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    let mut app = App::new(window).await;
    let mut now = Instant::now();
    event_loop.run(move |event, _, control_flow| {
        app.on_event(&event, control_flow, &mut now)
    });
}

fn main() {
    pollster::block_on(run());
}
