/// Define the data structures and traits that we need to render triangles
/// onto the screen.
use image::GenericImageView;
use cgmath::*;

/// The vertex is the thing that is a node in our mesh. It's what we build
/// meshes out of. In this case the Vertex is simple and it's only job is
/// to be part of a triangle.
/// On the computing side of things this will be a lot mode complicated, as
/// mesh traversal is not really a thing here.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Vertex {
    pub position: Vector3<f32>,
    pub texture_coords: Vector2<f32>,
    pub normal: Vector3<f32>,
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
/// Notes
/// -----
/// Meshes is not used here as meshes in the Simulation environment have a particular meaning
/// as the simulation domain. Simulation results may then be turned into Surfaces
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
    pub fallback_color: Vector3<u8>,
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
}

impl Texture {
    /// the format of the depth texture
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    /// Create the bind group descriptor to be used when creating the actual bind group
    fn desc(&self, layout: &wgpu::BindGroupLayout) -> wgpu::BindGroupDescriptor {
        wgpu::BindGroupDescriptor {
            label: Some("Texture bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.view)
                },
                 wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler)
                },
            ]
        }
    }
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
    pub fn create_bind_group(&self, device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> wgpu::BindGroup {
        device.create_bind_group(&self.desc(layout))
    }


    /// Create both the bind group and the layout.
    ///
    /// Notes
    /// -----
    /// The layout can be reused for BindGroups of other instances of a Texture 
    pub fn create_bind_group_with_layout(&self, device: &wgpu::Device) -> (wgpu::BindGroupLayout, wgpu::BindGroup) {
        let layout = device.create_bind_group_layout(&Texture::desc_layout());
        let bind_group = device.create_bind_group(&self.desc(&layout));
        return (layout, bind_group)

    }

    /// load an image from bytes in memory
    #[allow(dead_code)]
    pub fn from_bytes(
        name: String,
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
        let size = wgpu::Extent3d{
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };
        // create the texture and the sampler
        let texture = device.create_texture(&wgpu::TextureDescriptor{
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
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
        Ok(Self{ name: label.to_string(), texture, view, sampler})
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
        Self { name: label.to_string(), texture, view, sampler }
    }
}

/// A single object, will often consist of many different meshes that are combined.
/// For this reason, we will also define an model, that consists of meshes, together
/// with textures (one for each mesh)
pub struct Object {
    pub name: String,
    pub surfaces: Surface,
    pub texture: Texture,
}
