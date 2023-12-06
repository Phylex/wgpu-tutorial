/// The indexing that works for Vertices also kinda works for whole meshes.
/// This allows us to easily (and while only using an additional 16 numbers) to
/// create multiple copies (or instances) of the same mesh, without
/// needing to store every vertex for every mesh multiple times. 
/// The instance stores the information to place a given mesh into the world 
///
/// To actually use instances, there are two things we can do, the first
/// is to have all instances in a Uniform buffer, and then have the shaders
/// index into that uniform to get the required transformation matrix.
/// 
/// The other approach (which is used here) is to have the GPU pass the
/// instance matrix as a kind of Vertex attribute. With this approach the
/// GPU driver manages the distribution of the memory to the shader invocations.
pub struct Instance {
    /// define the position in the world that the mesh needs to be moved to
    pub position: Vector3<f32>,
    /// the rotation that needs to be performed to bring transfor
    /// the orientation of the mesh in 'model space' to the one in world space
    pub rotation: Quaternion<f32>,
    /// this allows us to grow/shrink our the instances of our mesh
    pub scale: Vector3<f32>,
}


impl Instance {
    /// turn the data in our shader struct into a matrix in homogenious
    /// coordinates
    pub fn compute_instance_matrix(&self) -> [[f32; 4]; 4] {
        let scale_matrix: Matrix4<f32> = Matrix4::<f32>::new(
            self.scale.x, 0.0, 0.0, 0.0,
            0.0, self.scale.y, 0.0, 0.0,
            0.0, 0.0, self.scale.z, 0.0,
            0.0, 0.0,          0.0, 1.0);
        (Matrix4::<f32>::from_translation(self.position) *
        (scale_matrix *
         Matrix4::<f32>::from(self.rotation))).into()
    }

    /// we need the buffer layout for this at one point so we encode it here
    /// as part of the instance implementation (its the equivalent of a static
    /// method)
    pub fn describe_layout<'a>() -> wgpu::VertexBufferLayout<'a> {
                use std::mem;
        wgpu::VertexBufferLayout {
            // we know that the instance transform matrix will bbe 
            array_stride: mem::size_of::<[[f32; 4]; 4]>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            // So the 4x4 matrix needs to be split into vectors (as we can't describe
            // matrices as vertex attributes, so we split the matrix into 4 vectors
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}
