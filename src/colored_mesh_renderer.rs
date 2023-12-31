/// The rendeder that will be used to render colorful wireframes of meshes
use wgpu::RenderPipelineDescriptor;

// This renderer depends on the data structures as defined in the model and instance 
use crate::{renderer, model, instance};

impl renderer::DescribeRenderPipeline for ColoredMeshRenderer {
    fn describe_color_attachment(view: Option<&wgpu::TextureView>) -> Option<wgpu::RenderPassColorAttachment> {
        match view {
            Some(view) => Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.001, g: 0.001, b: 0.001, a: 1.0 }),
                    store: wgpu::StoreOp::Store }
            }),
            None => None
        }
    }

    fn describe_depth_stencil(view: Option<&wgpu::TextureView>) -> Option<wgpu::RenderPassDepthStencilAttachment> {
        match view {
            Some(view) => Some(wgpu::RenderPassDepthStencilAttachment {
                view,
                depth_ops: Some(wgpu::Operations { 
                load: wgpu::LoadOp::Clear(1.0),
                store: wgpu::StoreOp::Store }),
                stencil_ops: None,
            }),
            None => None
        }
    }

    fn describe_render_pass<'a, 'b>(
        color_attachment_views: &'a[Option<wgpu::RenderPassColorAttachment<'b>>],
        depth_stencil_view: Option<wgpu::RenderPassDepthStencilAttachment<'b>>,
    ) -> wgpu::RenderPassDescriptor<'a, 'b> where 'a: 'b {
        wgpu::RenderPassDescriptor {
            label: Some("Color mesh renderer render pass"),
            color_attachments: color_attachment_views,
            depth_stencil_attachment: depth_stencil_view,
            timestamp_writes: None,
            occlusion_query_set: None,
        }
    }
}

impl <'a, 'b, 'c> model::DrawMesh<'a, 'b, 'c> for ColoredMeshRenderer {
    fn draw_mesh (
        render_pass: &'a mut wgpu::RenderPass<'b>,
        mesh: &'c model::Mesh,
        camera_bind_group: &'c wgpu::BindGroup,
    ) where 'b: 'a, 'c: 'b {
        ColoredMeshRenderer::draw_mesh_instanced(render_pass, mesh, 0..1, camera_bind_group); 
    }

    fn draw_mesh_instanced(
        render_pass: &'a mut wgpu::RenderPass<'b>,
        mesh: &'c model::Mesh,
        instances: std::ops::Range<u32>,
        camera_bind_group: &'c wgpu::BindGroup,
    ) where 'b: 'a, 'c: 'b {
        let mesh_texture_bind_group = mesh.material.as_ref().clone().unwrap().bind_group.as_ref().unwrap();
        render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.set_bind_group(0, camera_bind_group, &[]);
        render_pass.set_bind_group(1, mesh_texture_bind_group, &[]);
        render_pass.draw_indexed(0..mesh.num_elements, 0, instances);
    }
}

pub struct ColoredMeshRenderer {
    pub pipeline: wgpu::RenderPipeline,
}

impl ColoredMeshRenderer {
    pub fn new(
        // The device on which we create the render pipeline
        device: &wgpu::Device,
        // this is the camera that we are going to use for this pipeline
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        // the configuration of the surface that the resulting texture is going to be rendered to.
        surface_config: & wgpu::SurfaceConfiguration,
        // the properties of the depth buffer if we have one, the depth buffer that needs to be
        // used is set during the render pass. Here we declare how the buffer is used by the render
        // pipeline
        depth_format: Option<wgpu::TextureFormat>
    ) -> ColoredMeshRenderer {
        // The shader is hard coded into the program binary. Here it is loaded from
        // the binary and compiled into a shader module for the specific GPU that we have.
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Normal Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("./shaders/color_shader.wgsl").into()),
        });

        // The layout for the pipeline. We only have an observer for this simple pipeline, that
        // means no light and only the camera bind group that we need to care about in the layout.
        let layout = device.create_pipeline_layout(& wgpu::PipelineLayoutDescriptor {
            label: Some("Layout of the Colored Mesh Renderer Bind Group"),
            bind_group_layouts: &[camera_bind_group_layout],
            push_constant_ranges: &[],
        });

        // the stuff that concerns the Vertex shader, 
        let vertex_state = wgpu::VertexState {
            // a reference to the compiled shader
            module: &shader,
            // entry point for the vertex shader (the function that should is defined in the shader
            // source code that should be executed as the vertex shader).
            entry_point: "vs_main",
            // the layout of the Vertex and Instance in GPU memory
            buffers: &[model::Vertex::desc(), instance::Instance::desc()],
        };

        // describes attributes of the data in the vertex buffer so that the fixed function
        // hardware can make the right choices in sending data to the fragment shader 
        // this is the stage where the 'rendering primitives' are generated from the list of
        // vertices, hence the name.
        let primitive = wgpu::PrimitiveState {
            // describes how the individual vertices form triangles (or if they form points or
            // lines
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            // the list of triangle vertices is given in counter clockwise order, which determins
            // which side the normal (and thus the 'front face' of the triangle lies on
            front_face: wgpu::FrontFace::Ccw,
            // we can decide here that we either want the hardware to pass all triangles to the
            // rasterization stage, or only the ones with the front face facing 'the camear' or
            // those with the back face 'facing the camera', If a primitive is 'culled' it is not
            // sent to the fragment stage
            cull_mode: None,
            // if this is set to false, the triangles that are rendered need to be inside the [0-1]
            // x,y and range.
            unclipped_depth: false,
            // this pipeline should render objects as wiremeshes in a particular color. for this to
            // this is why we need to set this to polygon line mode, as then it does not fill the
            // triangles, but only draws lines around the triangles.
            polygon_mode: wgpu::PolygonMode::Line,
            // determins if every pixel touched by the triangle will be passed to the fragment
            // shader.
            conservative: false,
        };

        let fragent_state = wgpu::FragmentState {
            // here the same shader module (compiled binary) contains both the fragment and the
            // vertex shader code
            module: &shader,
            // the fragment shader has a different entry point than the vertex shader of course
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_config.format,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent::REPLACE,
                    alpha: wgpu::BlendComponent::REPLACE,
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],

        };

        // This determins if and how a Depth buffer will be used in the pipeline.
        let depth_stencil = depth_format.map(|format| wgpu::DepthStencilState {
            format,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        });

        // this determins if and how multisampling is performed (in multisampling each pixel is
        // split into multiple subpixels that are computed indipendently, the resulting color is a
        // mixture of the supersampled pixels
        let multisample_state = wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        };

        let descriptor = RenderPipelineDescriptor{
            label: Some("Colored Mesh Renderer"),
            layout: Some(&layout),
            vertex: vertex_state, 
            primitive,
            depth_stencil,
            multisample: multisample_state,
            fragment: Some(fragent_state),
            multiview: None,
        };
        ColoredMeshRenderer{ pipeline: device.create_render_pipeline(&descriptor)}
    }
}
