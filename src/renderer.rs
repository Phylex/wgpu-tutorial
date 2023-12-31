/// A render pipeline manager that keeps track of different render pipelines in the same place.


/// The actual rendering engine. At it's core will be the PipelineDescriptor that will be added to
/// the PipelineController. A pipeline is priciply coupled to a shader, so the base Type will be a
/// type called PipelineWithSource. The pipeline with source will be generic over the Type of the
/// pipeline as we can describe both a compute and a render pipeline

pub trait DescribeRenderPipeline {
    fn describe_color_attachment(view: Option<&wgpu::TextureView>) -> Option<wgpu::RenderPassColorAttachment>;
    fn describe_depth_stencil(view: Option<&wgpu::TextureView>) -> Option<wgpu::RenderPassDepthStencilAttachment>;
    fn describe_render_pass<'att_list, 'attachment> (
        color_attachment_views: &'att_list [Option<wgpu::RenderPassColorAttachment<'attachment>>],
        depth_stencil_view: Option<wgpu::RenderPassDepthStencilAttachment<'attachment>>,
    ) -> wgpu::RenderPassDescriptor<'att_list, 'attachment> where 'att_list: 'attachment ;
}
