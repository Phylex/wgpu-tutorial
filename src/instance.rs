use cgmath::{Vector3, Matrix4, Vector4, Quaternion};

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
    /// for our colored mesh renderer, we need the color of the mesh
    pub color: Vector4<f32>,

    pub buffer: Arc<Mutex<wgpu::Buffer>>,
    
    pub buffer_index: usize,
}

pub type RawInstance = [[f32;4];5];

impl Instance {
    /// turn the data in our shader struct into a matrix in homogenious
    /// coordinates
    pub fn compute_instance_matrix(&self) -> RawInstance {
        let buffer_content: [[f32; 4]; 4] = (
            Matrix4::<f32>::from_translation(self.position) *
            Matrix4::<f32>::from(self.rotation) *
            Matrix4::<f32>::new(
                self.scale.x, 0.0, 0.0, 0.0,
                0.0, self.scale.y, 0.0, 0.0,
                0.0, 0.0, self.scale.z, 0.0,
                0.0, 0.0,          0.0, 1.0)).into();
        let color: [f32; 4] = self.color.into();
        {
            let mut whole = [[0.0; 4]; 5];
            let (left, right) = whole.split_at_mut(buffer_content.len());
            left.copy_from_slice(&buffer_content);
            right.copy_from_slice(&[color]);
            whole
        }
    }

    /// rotate the instance by the given quaternion
    pub fn rotate(&mut self, rotation: Quaternion<f32>) {
        self.rotation = self.rotation * rotation;
    }

    /// translate the instance along the given vector
    pub fn translate(&mut self, translation: Vector3<f32>) {
        self.position += translation
    }
    
    /// we need the buffer layout for this at one point so we encode it here
    /// as part of the instance implementation (its the equivalent of a static
    /// method)
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
                use std::mem;
        wgpu::VertexBufferLayout {
            // we know the size of the instance transform matrix, and then we add the size of the
            // rgba color to the total size
            array_stride: (mem::size_of::<[[f32; 4]; 4]>() + mem::size_of::<Vector4<u8>>()) as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            // So the 4x4 matrix needs to be split into vectors (as we can't describe
            // matrices as vertex attributes, so we split the matrix into 4 vectors
            attributes: &[
                // the four vectors that together make up the transformation matrix for the
                // instance
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
                // the color encoded as 4 integers in the CPU and coverted to 4 floats [0,1] (rgba)
                // in the shader
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 16]>() as wgpu::BufferAddress,
                    shader_location: 9,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// many instances share the same buffer
pub struct InstanceBuffer<T> {
    cpu_buffer: Vec<T>,
    gpu_buffer: wgpu::Buffer,
    gpu_buffer_size: usize,
}

impl InstanceBuffer<T> {
    pub fn new() -> Self {
    }
}
