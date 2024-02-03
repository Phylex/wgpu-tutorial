use std::io::{BufReader, Cursor};

use std::sync::Arc;

use crate::model;

pub async fn load_string(file_name: &str) -> anyhow::Result<String> {
    let path = std::path::Path::new(env!("OUT_DIR"))
        .join("res")
        .join(file_name);
    println!("file_name: {:?}", path);
    let txt = std::fs::read_to_string(path)?;
    Ok(txt)
}

pub async fn load_binary(file_name: &str) -> anyhow::Result<Vec<u8>> {
    let path = std::path::Path::new(env!("OUT_DIR"))
        .join("res")
        .join(file_name);
    println!("binary file_name: {:?}", path);
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
    if let Ok(obj_materials) = obj_materials {
        for m in obj_materials.iter() {
            // get the texture for that material
            if let Some(diffuse_texture) = &m.diffuse_texture {
                let mut diffuse_texture = load_texture(diffuse_texture, device, queue).await?;
                diffuse_texture.add_bind_group(device);
                materials.push(Arc::new(diffuse_texture))
            }
        }
    }

    let meshes = models.into_iter().enumerate().map(|(o, m)| {
        // we always load the position of te vertices
        let mut vertices = (0..m.mesh.positions.len() / 3).map(|i| model::RawVertex{
            pos: [
                m.mesh.positions[i*3],
                m.mesh.positions[i*3+1],
                m.mesh.positions[i*3+2],
            ],
            tex_ccord: [0.0, 0.0],
            norm: [0.0, 0.0, 0.0],
        }).collect::<Vec<_>>();
        if m.mesh.texcoords.len() / 2 == m.mesh.positions.len() {
            for (i, v) in vertices.iter_mut().enumerate() {
                v.tex_ccord = [m.mesh.texcoords[i * 2], m.mesh.texcoords[i * 2 + 1]];
            }
        }
        if m.mesh.normals.len() / 3 == m.mesh.positions.len() {
            for (i, v) in vertices.iter_mut().enumerate() {
                v.norm = [m.mesh.normals[i * 3], m.mesh.normals[i * 3 + 1], m.mesh.normals[i * 3 + 1]];
            }
        }


        let mesh_material = match m.mesh.material_id {
            Some(id) => {
                if materials.len() > id {
                    Some(materials[id].clone())
                } else {
                    None
                }
            }
            None => None,
        };

        model::Surface::new(format!("{} surface no {}", file_name.to_string(), o), &vertices, &m.mesh.indices[..], mesh_material, device, queue)
    }).collect::<Vec<_>>();
    Ok(model::Object { 
        name: "SomeObject".to_string(),
        meshes,
    })
}

