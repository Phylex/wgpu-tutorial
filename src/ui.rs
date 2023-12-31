/// This is where the UI parts of the struct will be collected
use egui::Context;
use egui_winit::State;
use egui_wgpu::renderer::{Renderer, ScreenDescriptor};
use winit::window::Window;

use crate::instance::Instance;

pub struct UI {
    context: Context,
    pub state: State,
    renderer: Renderer,
    screen_descriptor: ScreenDescriptor,
}

impl UI {
    pub fn new(
        window: &Window,
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        screen_width: u32,
        screen_height: u32,
    ) -> Self {
        let context = Context::default();
        let state = State::new(
            context.viewport_id(),
            &window,
            Some(window.scale_factor() as f32),
            None
        );
        let renderer = Renderer::new(
            device,
            surface_format,
            Some(depth_format),
            1
        );
        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [screen_width, screen_height],
            pixels_per_point: 2.
        };
        Self {
            context,
            state,
            renderer,
            screen_descriptor,
        }
    }

    pub fn on_window_event(&self, event: &winit::event::WindowEvent) -> bool {
        let response = self.state.on_window_event(&self.context, event);
        response.consumed
    }

    pub fn generate_ui(
        &self,
        window: &Window,
        instance: &mut Instance,
        queue: &wgpu::Queue) -> {
        let raw_input = self.state.take_egui_input(window);
        let output = self.context.run(raw_input, |ctx| {
            egui::Window::new("Instance Controls").show(&ctx, |ui| {
                ui.label("Hello world!");
                if ui.button("Change Color").clicked() {
                    if instance.color.x == 1. {
                        instance.color.x = 0.;
                    } else {
                        instance.color.x = 1.;
                    }
                }
                queue.write_buffer(&self.instance_buffer, 0, &bytemuck::cast_slice(&self.model_instance.compute_instance_matrix()));
            });
            });
        });
    }
}
