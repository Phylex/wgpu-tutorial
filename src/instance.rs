use cgmath::{Vector3, Matrix4, Vector4, Quaternion};
use std::rc::{Rc, Weak};
use std::mem;
use std::sync::Arc;
use std::sync::Mutex;
use wgpu;

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

    pub buffer: Arc<Mutex<InstanceBuffer>>,
    
    pub buffer_index: Rc<usize>,
}

pub type RawInstance = [[f32;4];5];

impl Instance {
    /// Create a new instance given a new instance buffer
    pub fn new(ibuf: Arc<Mutex<InstanceBuffer>>) -> Self {
        let bi = ibuf.lock().unwrap().get_instance_buffer_slot();
        Self {
            position: Vector3{ x: 0.0, y: 0.0, z: 0.0 },
            rotation: Quaternion { v: Vector3 { x: 0.0, y: 0.0, z: 1.0 }, s: 0.0 },
            scale: Vector3 { x: 1.0, y: 1.0, z: 1.0 },
            color: Vector4 { x: 0.0, y: 1.0, z: 0.0, w: 1.0 },
            buffer: ibuf,
            buffer_index: bi,
        }
    }
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
            // we know te size of the instance transform matrix, and then we add the size of the
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


/// many instances share the same buffer the buffer will grow in powers o
/// so instance buffers will not be terribly large so we can keep a copy on the cpu side
pub struct InstanceBuffer {
    cpu_copy: Vec<RawInstance>,
    gpu_buffer: wgpu::Buffer,
    gpu_buffer_size: usize,
    handles: Vec<Weak<usize>>,
    occupied_slots: usize,
    changed: bool
}

impl InstanceBuffer {
    pub fn new(device: &wgpu::Device, buffer_size_in_elems: usize) -> Self {
        InstanceBuffer {
            cpu_copy: Vec::new(),
            handles: Vec::new(),
            gpu_buffer: Self::create_new_buffer_with_size(buffer_size_in_elems, device),
            gpu_buffer_size: 10,
            occupied_slots: 0,
            changed: false,
        }
    }

    fn create_new_buffer_with_size(size: usize, device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer(
            &wgpu::BufferDescriptor {
                label: Some("Instance Buffer on GPU"),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
                size: (mem::size_of::<RawInstance>() as usize * size) as wgpu::BufferAddress
            }
        )
    }

    fn get_occupied_slots(&self) -> Vec<usize> {
        self.handles.iter().filter_map(|h| h.upgrade()).map(|h| *h).collect()
    }


    fn get_first_free_slot_idx(&self) -> usize {
        let mut free_slot = self.handles.len();
        for (i, h) in self.handles.iter().enumerate() {
            match h.upgrade() {
                Some(ocidx) => continue,
                None => { free_slot = i; break; }
            }
        }
        free_slot
    }

    pub fn get_instance_buffer_slot(&mut self) -> Rc<usize> {
        let lowest_free_index = self.get_first_free_slot_idx();
        if lowest_free_index >= self.cpu_copy.len() {
            self.cpu_copy.push(RawInstance::default());
        }
        self.changed = true;
        Rc::new(lowest_free_index)
        
    }

    pub fn set_data(&mut self, instance: &Instance) {
        self.changed = true;
        self.cpu_copy[*instance.buffer_index] = instance.compute_instance_matrix();
    }

    /// all the interaction between the cpu and gpu happens here, when the cpu managed buffer
    /// is flushed to the GPU
    pub fn flush(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        // if nothing has changed, do nothing
        if !self.changed {
            return
        }
        // if by any chance the CPU buffer is bigger than the GPU buffer, resize the GPU buffer
        if self.cpu_copy.len() >= self.gpu_buffer_size {
            self.gpu_buffer_size = self.gpu_buffer_size * 2;
            self.gpu_buffer = Self::create_new_buffer_with_size(self.gpu_buffer_size, device) 
        }
        // get all the slots that actually have data and fill them into a contiguous buffer
        let occupied_indices = self.get_occupied_slots();
        self.occupied_slots = occupied_indices.len();
        let mut contiguous_instance_buffer: Vec<RawInstance> = vec![RawInstance::default(); self.gpu_buffer_size];
        for (i, &cpu_buf_idx) in  occupied_indices.iter().enumerate() {
            contiguous_instance_buffer[i] = self.cpu_copy[cpu_buf_idx];
        }
        queue.write_buffer(&self.gpu_buffer, 0, bytemuck::cast_slice(&contiguous_instance_buffer));
        self.changed = false;
    }
}
