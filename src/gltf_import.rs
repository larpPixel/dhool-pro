use crate::asset::ShadowCasterClass;
use crate::math::{Color, Mat4, Vec2, Vec3};
use crate::scene::{Material, MaterialClass, MaterialFlags, Mesh, TextureImage, Vertex};
use std::path::Path;
use std::{error::Error, fmt};

#[derive(Clone, Debug, PartialEq)]
pub enum GltfImportError {
    Gltf(String),
    UnsupportedImageFormat(String),
    MissingPositions { mesh: String },
    InvalidIndexCount { mesh: String, count: usize },
}

impl fmt::Display for GltfImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gltf(message) => write!(formatter, "glTF import failed: {message}"),
            Self::UnsupportedImageFormat(format) => {
                write!(formatter, "unsupported glTF image format: {format}")
            }
            Self::MissingPositions { mesh } => write!(formatter, "{mesh} has no positions"),
            Self::InvalidIndexCount { mesh, count } => {
                write!(formatter, "{mesh} has {count} indices, not a triangle list")
            }
        }
    }
}

impl Error for GltfImportError {}

pub fn import_gltf(path: impl AsRef<Path>) -> Result<(Vec<Material>, Vec<Mesh>), GltfImportError> {
    let (document, buffers, images) =
        gltf::import(path).map_err(|error| GltfImportError::Gltf(error.to_string()))?;

    let mut materials: Vec<Material> = document
        .materials()
        .map(|material| import_material(&material, &images))
        .collect::<Result<_, _>>()?;
    if materials.is_empty() {
        materials.push(default_material(0));
    }

    let mut meshes = Vec::new();
    for node in document.nodes() {
        let Some(source_mesh) = node.mesh() else {
            continue;
        };
        let transform = Mat4 {
            m: node.transform().matrix(),
        };
        for (primitive_index, primitive) in source_mesh.primitives().enumerate() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
            let positions: Vec<_> = reader
                .read_positions()
                .ok_or_else(|| GltfImportError::MissingPositions {
                    mesh: source_mesh.name().unwrap_or("unnamed").to_string(),
                })?
                .collect();
            let normals: Vec<_> = reader
                .read_normals()
                .map(Iterator::collect)
                .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);
            let texcoords: Vec<_> = reader
                .read_tex_coords(0)
                .map(|coords| coords.into_f32().collect())
                .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);
            let tangents: Vec<_> = reader
                .read_tangents()
                .map(Iterator::collect)
                .unwrap_or_else(|| vec![[1.0, 0.0, 0.0, 1.0]; positions.len()]);
            let colors: Vec<_> = reader
                .read_colors(0)
                .map(|colors| colors.into_rgba_f32().collect())
                .unwrap_or_else(|| vec![[1.0, 1.0, 1.0, 1.0]; positions.len()]);

            let vertices = positions
                .iter()
                .enumerate()
                .map(|(index, position)| {
                    let normal = normals.get(index).copied().unwrap_or([0.0, 1.0, 0.0]);
                    let uv = texcoords.get(index).copied().unwrap_or([0.0, 0.0]);
                    let tangent = tangents.get(index).copied().unwrap_or([1.0, 0.0, 0.0, 1.0]);
                    let color = colors.get(index).copied().unwrap_or([1.0, 1.0, 1.0, 1.0]);
                    Vertex {
                        position: Vec3::new(position[0], position[1], position[2]),
                        normal: Vec3::new(normal[0], normal[1], normal[2]),
                        tangent: Vec3::new(tangent[0], tangent[1], tangent[2]),
                        tangent_sign: tangent[3],
                        uv: Vec2::new(uv[0], uv[1]),
                        color: Color::new(color[0], color[1], color[2], color[3]),
                    }
                })
                .collect();

            let flat_indices: Vec<u32> = reader
                .read_indices()
                .map(|indices| indices.into_u32().collect())
                .unwrap_or_else(|| (0..positions.len() as u32).collect());
            if !flat_indices.len().is_multiple_of(3) {
                return Err(GltfImportError::InvalidIndexCount {
                    mesh: source_mesh.name().unwrap_or("unnamed").to_string(),
                    count: flat_indices.len(),
                });
            }
            let indices = flat_indices
                .chunks_exact(3)
                .map(|triangle| [triangle[0], triangle[1], triangle[2]])
                .collect();
            let material_index = primitive.material().index().unwrap_or(0);
            let reflective = materials
                .get(material_index)
                .is_some_and(|material| material.specular_intensity > 0.5);
            meshes.push(Mesh {
                name: format!(
                    "{}_{primitive_index}",
                    source_mesh.name().unwrap_or("gltf_mesh")
                ),
                vertices,
                indices,
                material_index,
                transform,
                shadow_class: ShadowCasterClass::Main,
                is_floor_or_ground: false,
                prepass_eligible: false,
                reflection_proxy: reflective,
            });
        }
    }
    Ok((materials, meshes))
}

fn import_material(
    source: &gltf::Material<'_>,
    images: &[gltf::image::Data],
) -> Result<Material, GltfImportError> {
    let pbr = source.pbr_metallic_roughness();
    let factor = pbr.base_color_factor();
    let metallic = pbr.metallic_factor();
    let roughness = pbr.roughness_factor();
    let texture = pbr
        .base_color_texture()
        .map(|texture| texture_from_gltf(&images[texture.texture().source().index()]))
        .transpose()?;
    let normal_texture = source
        .normal_texture()
        .map(|texture| texture_from_gltf(&images[texture.texture().source().index()]))
        .transpose()?;
    let pbr_texture = pbr
        .metallic_roughness_texture()
        .map(|texture| texture_from_gltf(&images[texture.texture().source().index()]))
        .transpose()?;
    let texture_count = usize::from(texture.is_some())
        + usize::from(pbr.metallic_roughness_texture().is_some())
        + usize::from(source.normal_texture().is_some())
        + usize::from(source.occlusion_texture().is_some());
    let class = if metallic > 0.5 {
        MaterialClass::Metal
    } else {
        MaterialClass::Default
    };
    Ok(Material {
        name: source.name().unwrap_or("gltf_material").to_string(),
        class,
        base_color: Color::new(factor[0], factor[1], factor[2], factor[3]),
        base_color_texture: texture,
        normal_texture,
        pbr_texture,
        roughness,
        metallic,
        specular_intensity: 0.25 + metallic * 0.65,
        cavity: 1.0,
        material_id: source.index().unwrap_or(0).min(255) as u8,
        texture_sample_count: texture_count.clamp(1, 4),
        flags: MaterialFlags {
            receives_contact_shadows: true,
            uses_bent_normals: true,
            ..MaterialFlags::default()
        },
        emission: Color::BLACK,
    })
}

fn default_material(index: usize) -> Material {
    Material {
        name: "gltf_default".to_string(),
        class: MaterialClass::Default,
        base_color: Color::rgb(0.7, 0.7, 0.7),
        base_color_texture: None,
        normal_texture: None,
        pbr_texture: None,
        roughness: 0.7,
        metallic: 0.0,
        specular_intensity: 0.25,
        cavity: 1.0,
        material_id: index.min(255) as u8,
        texture_sample_count: 1,
        flags: MaterialFlags {
            receives_contact_shadows: true,
            uses_bent_normals: true,
            ..MaterialFlags::default()
        },
        emission: Color::BLACK,
    }
}

fn texture_from_gltf(image: &gltf::image::Data) -> Result<TextureImage, GltfImportError> {
    use gltf::image::Format;
    let pixels = match image.format {
        Format::R8G8B8 => image
            .pixels
            .chunks_exact(3)
            .map(|pixel| Color::rgb(byte(pixel[0]), byte(pixel[1]), byte(pixel[2])))
            .collect(),
        Format::R8G8B8A8 => image
            .pixels
            .chunks_exact(4)
            .map(|pixel| {
                Color::new(
                    byte(pixel[0]),
                    byte(pixel[1]),
                    byte(pixel[2]),
                    byte(pixel[3]),
                )
            })
            .collect(),
        other => {
            return Err(GltfImportError::UnsupportedImageFormat(format!(
                "{other:?}"
            )))
        }
    };
    Ok(TextureImage {
        width: image.width,
        height: image.height,
        pixels,
    })
}

fn byte(value: u8) -> f32 {
    value as f32 / 255.0
}
