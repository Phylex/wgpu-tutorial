use std::sync::{Arc, Mutex};
use core::ops::Range;
use wgpu::util::DeviceExt;

/// Define the data structures and traits that we need to render triangles
/// onto the screen.
use image::{GenericImageView, Rgba, ImageBuffer};
use cgmath::*;

use crate::instance;

/// The vertex is the thing that is a node in our mesh. It's what we build
/// meshes out of. In this case the Vertex is simple and it's only job is
/// to be part of a triangle.
/// On the computing side of things this will be a lot mode complicated, aso
/// mesh traversal is not really a thing here.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Vertex {
    pub position: Vector3<f32>,
    pub texture_coords: Vector2<f32>,
    pub normal: Vector3<f32>,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RawVertex {
    pub pos: [f32; 3],
    pub tex_ccord: [f32; 2],
    pub norm: [f32; 3],
}

impl From<Vertex> for RawVertex {
    fn from(value: Vertex) -> Self {
        Self {
            pos: [value.position.x, value.position.y, value.position.z],
            tex_ccord: [value.texture_coords.x, value.texture_coords.y],
            norm: [value.normal.x, value.normal.y, value.normal.z]
        }
    }
}

impl From<RawVertex> for Vertex {
    fn from(value: RawVertex) -> Self {
        Self {
            position: value.pos.into(),
            texture_coords: value.tex_ccord.into(),
            normal: value.norm.into(),
        }
    }
}

// We need to convert to something that bytemuck can cast so that
// it can be written into a GPU buffer
impl From<Vertex> for [f32; 8] {
    fn from(value: Vertex) -> Self {
        [
            value.position.x,
            value.position.y,
            value.position.z,
            value.texture_coords.x,
            value.texture_coords.y,
            value.normal.x,
            value.normal.y,
            value.normal.z
        ]
    }
}

impl Vertex {
    /// describe the layout of the vertex data 
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            // this is the distance in the array between two vertices
            array_stride: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // vertex position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // texture coordinate
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // vertex normal
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 5]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}


/// The core data structure that defines the geometry of the 3D model is the Surface
/// also known as Mesh. A Surface consists of a list of vertices together with other vertex
/// attributes like normals or texture coordinates
/// 
pub struct Surface {
    pub name: String,
    /// This is where the data for the vertices is stored
    pub vertex_buffer: wgpu::Buffer,
    /// Many vertices are used multiple times in different triangles
    /// so to save memory the vertices with the attributes are stored
    /// only once and when building the triangles GPU iterates through
    /// the index buffer using the vertices referenced by the index in
    /// the index buffer.
    pub index_buffer: wgpu::Buffer,
    pub num_elements: u32,
    pub fallback_color: Vector4<f32>,
    pub instances: Vec<instance::Instance>,
    pub instance_buffer: instance::InstanceBuffer,
    // this is the index of a material used for this mesh
    pub material: Option<Arc<Texture>>,
}

impl Surface {
    pub fn new(
        name: String,
        vertices: &[RawVertex],
        indices: &[u32],
        material: Option<Arc<Texture>>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Self {
        let mut instbuf = instance::InstanceBuffer::new(&device, 5);
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor{
            label: Some(&format!("{:?} Vertex Buffer", name)),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{:?} Index Buffer", name)),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let mut first_instance = instance::Instance::new(instbuf.get_instance_buffer_slot());
        first_instance.update(&mut instbuf);
        instbuf.flush(device, queue);
        let instances = vec![first_instance];
        Self {
            name,
            vertex_buffer,
            index_buffer,
            num_elements: indices.len() as u32,
            material,
            fallback_color: [0., 1., 0., 1.].into(),
            instance_buffer: instbuf,
            instances
        }
    }

    pub fn create_instance(
        &mut self,
        position: Vector3<f32>, 
        rotation: Quaternion<f32>,
        scale: Vector3<f32>,
        // todo change to proper color space definition
        color: Vector4<f32>,
    ) {
        self.instances.push(
            instance::Instance::init(
                position,
                rotation,
                scale,
                color,
                self.instance_buffer.get_instance_buffer_slot(),
            )
        );
    }

    pub fn update_vertex_buffer(&mut self, vertices: &[RawVertex], queue: &wgpu::Queue) {
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(vertices));
    }

    pub fn update_index_buffer(&mut self, indices: &[usize], queue: &wgpu::Queue) {
        queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(indices));
    }
}

pub trait DrawMesh<'a, 'b, 'c> {
    fn draw_mesh(
        render_pass: &'a mut wgpu::RenderPass<'b>,
        mesh: &'c Surface,
        camera_bind_group: &'c wgpu::BindGroup,
    ) where 'b: 'a, 'c: 'b;
    fn draw_mesh_instanced(
        render_pass: &'a mut wgpu::RenderPass<'b>,
        mesh: &'c Surface,
        instances: Range<u32>,
        camera_bind_group: &'c wgpu::BindGroup,
    ) where 'b: 'a, 'c: 'b;
}


/**
To be able to render meshes with fancy images on their surface, we need a texture
This texture will hold the underlying image as well as the methods to get it into the
GPU.

Textures are however a lot more useful than simply for putting surfaces onto meshes
even though that's where they get their name. We are going to use a texture in the
implenetation of the z buffer algorithm to store the depth of the closest pixel as
a greyscale image
*/
pub struct Texture {
    pub name: String,
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub size: wgpu::Extent3d,
    pub bind_group_layout: Option<wgpu::BindGroupLayout>,
    pub bind_group: Option<wgpu::BindGroup>,
}

impl Texture {
    /// the format of the depth texture
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    /// Provide the Description of the texture on the GPU
    fn desc_layout() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            label: Some("Texture Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    count: None,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    }
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    count: None,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                }
            ]
        }
    }

    fn desc(label: Option<&str>, size: wgpu::Extent3d) -> wgpu::TextureDescriptor {
        wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        }
    }
    /// Create the bind group layout on the GPU. The layout needs to be known to the GPU driver
    ///
    /// Notes
    /// -----
    /// The management of this layout is left to the caller as the texture cannot know if there
    /// are multiple Textures which can share the same texture bind group layout.
    pub fn create_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&Texture::desc_layout())
    }

    /// Create the bind group with the texture resources.
    ///
    /// Notes
    /// -----
    /// The caller must make sure that this function is called with the bind group layout
    /// acquired by calling the create layout 
    pub fn create_bind_group(name: &str, view: &wgpu::TextureView, sampler: &wgpu::Sampler, device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> wgpu::BindGroup {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&(name.to_owned() + "bind Group")),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view)
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler)
                },
            ]
        })
    }

    pub fn update_gpu_texture(&self, queue: &wgpu::Queue, data: &ImageBuffer<Rgba<u8>, Vec<u8>>) {
        queue.write_texture(
            wgpu::ImageCopyTexture {
                aspect: wgpu::TextureAspect::All,
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * self.size.width),
                rows_per_image: Some(self.size.height),
            },
            self.size,
        );
    }
    
    pub fn add_bind_group(&mut self, device: &wgpu::Device) {
        let layout = device.create_bind_group_layout(&Texture::desc_layout());
        let bind_group = Texture::create_bind_group(&self.name, &self.view, &self.sampler, device, &layout);
        self.bind_group = Some(bind_group);
        self.bind_group_layout = Some(layout);

    }

    /// load an image from bytes in memory
    #[allow(dead_code)]
    pub fn from_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
        label: &str,
        ) -> anyhow::Result<Self> {
        let img = image::load_from_memory(bytes)?;
        Self::from_image(device, queue, &img, label)
    }

    /// Load a texture from an image 
    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        img: &image::DynamicImage,
        label: &str
    ) -> anyhow::Result<Self> {
        let rgba = img.to_rgba8();
        let dimensions = img.dimensions();
        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        // create the texture and the sampler
        let texture = device.create_texture(
            &Texture::desc(
                Some(label),
                size.clone()
            )
        );
        queue.write_texture(
            wgpu::ImageCopyTexture {
                aspect: wgpu::TextureAspect::All,
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            &rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            size,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let layout = Texture::create_layout(&device);
        let bind_group = Some(Texture::create_bind_group(label, &view, &sampler, device, &layout));
        Ok(Self{ size, name: label.to_string(), texture, view, sampler, bind_group_layout: Some(layout), bind_group})
    }
    
    /// create a depth texture
    pub fn create_depth_texture(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        label: &str
    ) -> Self {
        let size = wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: Some(label),
            size, 
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let texture = device.create_texture(&desc);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(
            &wgpu::SamplerDescriptor { 
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Nearest,
                compare: Some(wgpu::CompareFunction::LessEqual),
                lod_min_clamp: 0.0,
                lod_max_clamp: 100.0,
                ..Default::default()
            }
        );
        Self { size, name: label.to_string(), texture, view, sampler, bind_group_layout: None, bind_group: None}
    }
}

/// A single object, will often consist of many different meshes that are combined.
/// For this reason, we will also define an model, that consists of meshes, together
/// with textures (one for each mesh)
pub struct Object {
    pub name: String,
    pub meshes: Vec<Surface>,
}

impl Object {
    pub fn new(name: String) -> Self { 
        Self {
            name,
            meshes: Vec::new(),
        }
    }

    pub fn translate(&mut self, dx: Vector3<f32>) {

    }

    pub fn move_instance(&mut self, dx: Vector3<f32>, id: usize) {
    }
}
