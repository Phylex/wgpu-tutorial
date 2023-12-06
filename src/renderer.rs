/// A render pipeline manager that keeps track of different render pipelines in the same place.
/// Inspired by the Code in Blub by Andreas Reich
use std::{rc::{Rc, Weak}};

/// The thing that is 
pub type PipelineHandle = Rc<usize>;

/// The actual rendering engine. At it's core will be the PipelineDescriptor that will be added to
/// the PipelineController. A pipeline is priciply coupled to a shader, so the base Type will be a
/// type called PipelineWithSource. The pipeline with source will be generic over the Type of the
/// pipeline as we can describe both a compute and a render pipeline
struct RenderPipeline {
    pipeline: wgpu::RenderPipeline,
    handle: Weak<usize>,
}

/// The PipelineController stores different render Pipelines that can be accessed later in the
/// program
struct PipelineController {
    render_pipelines: Vec<RenderPipeline>,
}

impl PipelineController {
    pub fn new() -> Self {
        PipelineController { render_pipelines: Vec::new() }
    }

    pub fn create_new_pipeline(&mut self, device: wgpu::Device, descriptor: wgpu::RenderPipelineDescriptor) -> PipelineHandle {
    }
}
