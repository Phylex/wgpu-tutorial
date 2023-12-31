use std::io::{BufReader, Cursor};

use wgpu::util::DeviceExt;
use std::sync::Arc;

use crate::model;

pub async fn load_string(file_name: &str) -> anyhow::Result<String> {
    let path = std::path::Path::new(env!("OUT_DIR"))
        .join("res")
        .join(file_name);
    let txt = std::fs::read_to_string(path)?;
    Ok(txt)
}

pub async fn load_binary(file_name: &str) -> anyhow::Result<Vec<u8>> {
    let path = std::path::Path::new(env!("OUT_DIR"))
        .join("res")
        .join(file_name);
    let data = std::fs::read(path)?;
    Ok(data)
}


pub async fn load_texture (
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<model::Texture> {
    let data = load_binary(file_name).await?;
    model::Texture::from_bytes(device, queue, &data, file_name)
}

pub async fn load_model(
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<model::Object> {
    let obj_text = load_string(file_name).await?;
    let obj_cursor = Cursor::new(obj_text);
    let mut obj_reader = BufReader::new(obj_cursor);

    let (models, obj_materials) = tobj::load_obj_buf_async(
        &mut obj_reader,
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ..Default::default()
        },
        |p| async move {
            let mat_text = load_string(&p).await.unwrap();
            tobj::load_mtl_buf(&mut BufReader::new(Cursor::new(mat_text)))
        },
    ).await?;

    let mut materials = Vec::new();

    for m in obj_materials? {
        // get the texture for that material
        let mut diffuse_texture = load_texture(&m.diffuse_texture.unwrap_or_else(|| "Some Texture".to_string()), device, queue).await?;
        diffuse_texture.add_bind_group(device);
        // create the bind group for a given texture
        materials.push(Arc::new(diffuse_texture))
    }

    let meshes = models.into_iter().map(|m| {
        let vertices = (0..m.mesh.positions.len() / 3).map(|i| model::RawVertex{
            pos: [
                m.mesh.positions[i*3],
                m.mesh.positions[i*3+1],
                m.mesh.positions[i*3+2],
            ],
            tex_ccord: [m.mesh.texcoords[i * 2], m.mesh.texcoords[i * 2 + 1]],
            norm: [
                m.mesh.normals[i * 3],
                m.mesh.normals[i * 3 + 1],
                m.mesh.normals[i * 3 + 2],
            ],
        }).collect::<Vec<_>>();

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor{
            label: Some(&format!("{:?} Vertex Buffer", file_name)),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{:?} Index Buffer", file_name)),
            contents: bytemuck::cast_slice(&m.mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        model::Mesh {
            name: file_name.to_string(),
            vertex_buffer,
            index_buffer,
            num_elements: m.mesh.indices.len() as u32,
            material: Some(materials[m.mesh.material_id.unwrap_or(0)].clone()),
            fallback_color: [1., 1., 1., 1.].into(),
        }
    }).collect::<Vec<_>>();
    Ok(model::Object { 
        name: "SomeObject".to_string(),
        meshes,
    })
}

