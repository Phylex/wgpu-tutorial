use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{WindowBuilder, Window}, dpi::PhysicalSize,
};


// We need a place to put the objects/data related to the global state into
struct App {
    window: Window, // The winit Window
    // we need to keep the size here so that we can detect when the screen size
    // changes
    window_size: PhysicalSize<u32>,
    // the connection of the gpu with the window so that the GPU can draw stuff
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
            dx12_shader_compiler: wgpu::Dx12Compiler::Fxc
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
            features: wgpu::Features::empty(), // we are happy with the required features
            limits: wgpu::Limits::default(),
        };
        let (mut device, queue) = adapter.request_device(&device_descriptor, None).await.unwrap();
        
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
        App {
            window,
            window_size,
            surface,
            instance,
            adapter,
            device,
            queue,
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
    let app = App::new(window);
}

fn main() {
    pollster::block_on(run());
}
