// In this simple example there is no input to the shader other
// than a vertex index, that is used to pick the right vertex
// from the predefined one.
@vertex fn vertex_shader(
    @builtin(vertex_index) vertexIndex: u32
) -> @builtin(position) vec4f {
    let pos = array(
        vec2f(0.0, 0.5),
        vec2f(-0.5, -0.5),
        vec2f(-0.5, 0.5),
    );
    // These are homogenious coordinates
    return vec4f(pos[vertexIndex], 0.0, 1.0);
}

@fragment fn fragment_shader() -> @location(0) vec4f {
    return vec4f(0.0, 0.0, 1.0, 1.0);
}
