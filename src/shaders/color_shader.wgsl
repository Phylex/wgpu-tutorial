// We want to build a really simple shader. Each vertex has a point in 3D space and a color.
// This essentially means no lighting effects and fancy math in the fragment shader. Just a straight up
// 3D version of the OpenGL triangle kind of shader, that support Instances and a moving camera in 3D
struct Camera {
    view_proj: mat4x4<f32>,
};
 
@group(1) @binding(0)
var<uniform> camera: Camera;

struct VertexInput {
    @location(0) position: vec4<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

struct InstanceInput {
    @location(5) transform_matrix_0: vec4<f32>,
    @location(6) transform_matrix_1: vec4<f32>,
    @location(7) transform_matrix_2: vec4<f32>,
    @location(8) transform_matrix_3: vec4<f32>,
    @location(9) scale: vec4<f32>,
};

// Here the vertex shader is doing pretty boring stuff, it simply maps the points into the view volume
// via transforming 
@vertex
fn vs_main(
    model: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    let instance_transform = mat4x4<f32>(
        instance.transform_matrix_0,
        instance.transform_matrix_1,
        instance.transform_matrix_2,
        instance.transform_matrix_3,
    );

    var out: VertexOutput;
    
    // simply transform the point on the grid according to the instance transform
    var instanced_position: vec4<f32> = instance_transform * vec4<f32>(model.position, 1.0);

    // this is the thing that really matters to the clipping and rasterization process
    out.clip_position = camera.view_proj * instanced_position.xyz;
    out.color = model.color;
    return out;
}

// The fragment shader is really straight forward, as we essentially do no light calculations what so ever
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
