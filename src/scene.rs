use crate::asset::{
    MaterialDesc, MeshDesc, ShadowCasterClass, TextureDesc, TextureFormat, TextureRole,
};
use crate::gltf_import::{import_gltf, GltfImportError};
use crate::math::{Color, Mat4, Vec2, Vec3};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterialClass {
    Default,
    Concrete,
    Foliage,
    Skin,
    Hair,
    Glass,
    Metal,
    Emissive,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MaterialFlags {
    pub glass: bool,
    pub emissive: bool,
    pub receives_contact_shadows: bool,
    pub alpha_tested: bool,
    pub uses_bent_normals: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub name: String,
    pub class: MaterialClass,
    pub base_color: Color,
    pub base_color_texture: Option<TextureImage>,
    pub normal_texture: Option<TextureImage>,
    pub pbr_texture: Option<TextureImage>,
    pub roughness: f32,
    pub metallic: f32,
    pub specular_intensity: f32,
    pub cavity: f32,
    pub material_id: u8,
    pub texture_sample_count: usize,
    pub flags: MaterialFlags,
    pub emission: Color,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextureImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<Color>,
}

impl TextureImage {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, String> {
        let image = image::open(path.as_ref()).map_err(|error| error.to_string())?;
        let rgba = image.to_rgba8();
        Ok(Self {
            width: rgba.width(),
            height: rgba.height(),
            pixels: rgba
                .pixels()
                .map(|pixel| {
                    Color::new(
                        pixel[0] as f32 / 255.0,
                        pixel[1] as f32 / 255.0,
                        pixel[2] as f32 / 255.0,
                        pixel[3] as f32 / 255.0,
                    )
                })
                .collect(),
        })
    }

    pub fn sample_repeat(&self, uv: Vec2) -> Color {
        if self.width == 0 || self.height == 0 || self.pixels.is_empty() {
            return Color::WHITE;
        }
        let u = uv.x.rem_euclid(1.0);
        let v = uv.y.rem_euclid(1.0);
        let x = (u * (self.width - 1) as f32).round() as u32;
        let y = ((1.0 - v) * (self.height - 1) as f32).round() as u32;
        self.pixels[(y * self.width + x) as usize]
    }
}

impl Material {
    pub fn to_asset_desc(&self) -> MaterialDesc {
        let mut textures = vec![
            TextureDesc {
                name: format!("{}_normal_bent_bc7", self.name),
                format: TextureFormat::Bc7,
                role: TextureRole::NormalsAndBentNormals,
            },
            TextureDesc {
                name: format!("{}_base_ao_bc3", self.name),
                format: TextureFormat::Bc3,
                role: TextureRole::BaseColorAndAo,
            },
            TextureDesc {
                name: format!("{}_spec_rough_bc3", self.name),
                format: TextureFormat::Bc3,
                role: TextureRole::SpecularRoughnessHeight,
            },
        ];
        if self.flags.alpha_tested {
            textures.push(TextureDesc {
                name: format!("{}_opacity_bc4", self.name),
                format: TextureFormat::Bc4,
                role: TextureRole::Opacity,
            });
        }

        MaterialDesc {
            name: self.name.clone(),
            textures,
            texture_sample_count: self.texture_sample_count,
            runtime_texture_blending: false,
            requires_specular_occlusion: self.specular_intensity > 0.15,
            has_bent_normals: self.flags.uses_bent_normals,
            alpha_tested: self.flags.alpha_tested,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub tangent: Vec3,
    pub tangent_sign: f32,
    pub uv: Vec2,
    pub color: Color,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    pub name: String,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<[u32; 3]>,
    pub material_index: usize,
    pub transform: Mat4,
    pub shadow_class: ShadowCasterClass,
    pub is_floor_or_ground: bool,
    pub prepass_eligible: bool,
    pub reflection_proxy: bool,
}

impl Mesh {
    pub fn to_asset_desc(&self) -> MeshDesc {
        MeshDesc {
            name: self.name.clone(),
            material_index: self.material_index,
            is_floor_or_ground: self.is_floor_or_ground,
            shadow_class: self.shadow_class,
            participates_in_prepass: self.prepass_eligible,
            writes_motion_vectors_in_prepass: false,
            long_thin_triangles: false,
            dense_alpha_cards: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LightKind {
    Directional { direction: Vec3 },
    Local { position: Vec3, radius: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    pub name: &'static str,
    pub kind: LightKind,
    pub color: Color,
    pub intensity: f32,
    pub casts_shadow: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub position: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub vertical_fov_radians: f32,
    pub near: f32,
    pub far: f32,
}

impl Camera {
    pub fn view_projection(self, aspect: f32) -> Mat4 {
        Mat4::look_at_rh(self.position, self.target, self.up)
            * Mat4::perspective_rh(self.vertical_fov_radians, aspect, self.near, self.far)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderScene {
    pub camera: Camera,
    pub materials: Vec<Material>,
    pub meshes: Vec<Mesh>,
    pub lights: Vec<Light>,
    pub sky_color: Color,
    pub ambient_color: Color,
}

impl RenderScene {
    pub fn asset_descriptors(&self) -> (Vec<MaterialDesc>, Vec<MeshDesc>) {
        (
            self.materials.iter().map(Material::to_asset_desc).collect(),
            self.meshes.iter().map(Mesh::to_asset_desc).collect(),
        )
    }
}

pub fn build_demo_scene() -> RenderScene {
    let materials = vec![
        Material {
            name: "mat_concrete_floor".to_string(),
            class: MaterialClass::Concrete,
            base_color: Color::rgb(0.45, 0.43, 0.39),
            base_color_texture: None,
            normal_texture: None,
            pbr_texture: None,
            roughness: 0.82,
            metallic: 0.0,
            specular_intensity: 0.22,
            cavity: 0.8,
            material_id: 4,
            texture_sample_count: 3,
            flags: MaterialFlags {
                receives_contact_shadows: true,
                uses_bent_normals: true,
                ..MaterialFlags::default()
            },
            emission: Color::BLACK,
        },
        Material {
            name: "mat_brushed_gold".to_string(),
            class: MaterialClass::Metal,
            base_color: Color::rgb(1.0, 0.72, 0.28),
            base_color_texture: None,
            normal_texture: None,
            pbr_texture: None,
            roughness: 0.24,
            metallic: 1.0,
            specular_intensity: 0.9,
            cavity: 0.95,
            material_id: 32,
            texture_sample_count: 4,
            flags: MaterialFlags {
                receives_contact_shadows: true,
                uses_bent_normals: true,
                ..MaterialFlags::default()
            },
            emission: Color::BLACK,
        },
        Material {
            name: "mat_glass_blue".to_string(),
            class: MaterialClass::Glass,
            base_color: Color::rgb(0.35, 0.55, 0.9),
            base_color_texture: None,
            normal_texture: None,
            pbr_texture: None,
            roughness: 0.04,
            metallic: 0.0,
            specular_intensity: 1.0,
            cavity: 1.0,
            material_id: 64,
            texture_sample_count: 3,
            flags: MaterialFlags {
                glass: true,
                receives_contact_shadows: false,
                uses_bent_normals: true,
                ..MaterialFlags::default()
            },
            emission: Color::BLACK,
        },
        Material {
            name: "mat_neon".to_string(),
            class: MaterialClass::Emissive,
            base_color: Color::rgb(1.0, 0.2, 0.1),
            base_color_texture: None,
            normal_texture: None,
            pbr_texture: None,
            roughness: 0.2,
            metallic: 0.0,
            specular_intensity: 0.1,
            cavity: 1.0,
            material_id: 96,
            texture_sample_count: 2,
            flags: MaterialFlags {
                emissive: true,
                ..MaterialFlags::default()
            },
            emission: Color::rgb(4.0, 0.6, 0.25),
        },
    ];

    let ground = Mesh {
        name: "ground_floor_excluded_from_shadows".to_string(),
        vertices: quad_vertices(3.5, Vec3::Y, -0.8, Color::WHITE),
        indices: vec![[0, 1, 2], [0, 2, 3]],
        material_index: 0,
        transform: Mat4::IDENTITY,
        shadow_class: ShadowCasterClass::None,
        is_floor_or_ground: true,
        prepass_eligible: true,
        reflection_proxy: false,
    };

    let gold_panel = Mesh {
        name: "prepassed_reflective_panel".to_string(),
        vertices: upright_quad_vertices(1.2, 1.8, Color::WHITE),
        indices: vec![[0, 1, 2], [0, 2, 3]],
        material_index: 1,
        transform: Mat4::translation(Vec3::new(-0.9, 0.15, 0.2)),
        shadow_class: ShadowCasterClass::Main,
        is_floor_or_ground: false,
        prepass_eligible: true,
        reflection_proxy: true,
    };

    let glass_panel = Mesh {
        name: "glass_reflection_proxy".to_string(),
        vertices: upright_quad_vertices(1.0, 1.5, Color::WHITE),
        indices: vec![[0, 1, 2], [0, 2, 3]],
        material_index: 2,
        transform: Mat4::translation(Vec3::new(0.85, 0.0, -0.15)),
        shadow_class: ShadowCasterClass::ContactOnly,
        is_floor_or_ground: false,
        prepass_eligible: false,
        reflection_proxy: true,
    };

    let neon = Mesh {
        name: "small_emissive_reference".to_string(),
        vertices: upright_quad_vertices(0.45, 0.25, Color::WHITE),
        indices: vec![[0, 1, 2], [0, 2, 3]],
        material_index: 3,
        transform: Mat4::translation(Vec3::new(0.0, 0.85, 0.05)),
        shadow_class: ShadowCasterClass::None,
        is_floor_or_ground: false,
        prepass_eligible: false,
        reflection_proxy: true,
    };

    RenderScene {
        camera: Camera {
            position: Vec3::new(0.0, 1.3, 4.2),
            target: Vec3::new(0.0, 0.1, 0.0),
            up: Vec3::Y,
            vertical_fov_radians: 55.0_f32.to_radians(),
            near: 0.05,
            far: 25.0,
        },
        materials,
        meshes: vec![ground, gold_panel, glass_panel, neon],
        lights: vec![
            Light {
                name: "sun_key",
                kind: LightKind::Directional {
                    direction: Vec3::new(-0.35, -0.75, -0.45).normalize_or_zero(),
                },
                color: Color::rgb(1.0, 0.95, 0.86),
                intensity: 2.4,
                casts_shadow: true,
            },
            Light {
                name: "warm_local",
                kind: LightKind::Local {
                    position: Vec3::new(0.9, 1.2, 1.1),
                    radius: 4.0,
                },
                color: Color::rgb(1.0, 0.58, 0.35),
                intensity: 5.0,
                casts_shadow: true,
            },
        ],
        sky_color: Color::rgb(0.08, 0.12, 0.18),
        ambient_color: Color::rgb(0.035, 0.045, 0.055),
    }
}

pub fn build_asset_showcase(path: impl AsRef<Path>) -> Result<RenderScene, GltfImportError> {
    let mut scene = build_demo_scene();
    scene.camera.position = Vec3::new(0.0, 1.2, 5.4);
    scene.camera.target = Vec3::new(0.0, 0.15, 0.0);

    if let Some(panel) = scene
        .meshes
        .iter_mut()
        .find(|mesh| mesh.name == "prepassed_reflective_panel")
    {
        panel.transform = Mat4::translation(Vec3::new(-1.75, 0.1, -0.15));
    }
    if let Some(panel) = scene
        .meshes
        .iter_mut()
        .find(|mesh| mesh.name == "glass_reflection_proxy")
    {
        panel.transform = Mat4::translation(Vec3::new(1.75, 0.0, -0.2));
    }
    if let Some(neon) = scene
        .meshes
        .iter_mut()
        .find(|mesh| mesh.name == "small_emissive_reference")
    {
        neon.transform = Mat4::translation(Vec3::new(0.0, 1.65, -0.3));
    }

    scene.meshes.push(Mesh {
        name: "architectural_back_wall".to_string(),
        vertices: upright_quad_vertices(6.5, 4.0, Color::rgb(0.52, 0.56, 0.62)),
        indices: vec![[0, 1, 2], [0, 2, 3]],
        material_index: 0,
        transform: Mat4::translation(Vec3::new(0.0, 1.0, -1.0)),
        shadow_class: ShadowCasterClass::None,
        is_floor_or_ground: false,
        prepass_eligible: true,
        reflection_proxy: false,
    });

    let material_offset = scene.materials.len();
    let (materials, mut meshes) = import_gltf(path)?;
    for mesh in &mut meshes {
        mesh.material_index += material_offset;
        mesh.name = format!("web_asset_{}", mesh.name);
        mesh.transform = mesh.transform
            * Mat4::scale(Vec3::splat(0.72))
            * Mat4::translation(Vec3::new(0.0, 0.0, 0.15));
    }
    scene.materials.extend(materials);
    scene.meshes.extend(meshes);
    Ok(scene)
}

pub fn build_aaa_scene(root: impl AsRef<Path>) -> Result<RenderScene, GltfImportError> {
    let root = root.as_ref();
    let mut materials = vec![
        load_surface_material(
            root,
            "asphalt_02",
            "wet_asphalt",
            MaterialClass::Concrete,
            0.42,
            0.0,
            10,
        )?,
        load_surface_material(
            root,
            "concrete_floor",
            "worn_concrete",
            MaterialClass::Concrete,
            0.78,
            0.0,
            11,
        )?,
        load_surface_material(
            root,
            "brick_wall_08",
            "aged_brick",
            MaterialClass::Concrete,
            0.84,
            0.0,
            12,
        )?,
        load_surface_material(
            root,
            "metal_plate_02",
            "painted_metal",
            MaterialClass::Metal,
            0.48,
            1.0,
            13,
        )?,
    ];
    materials.push(emissive_material(
        "cyan_transit_neon",
        Color::rgb(0.08, 0.75, 1.0),
        Color::rgb(0.3, 5.5, 8.0),
        14,
    ));
    materials.push(emissive_material(
        "amber_service_neon",
        Color::rgb(1.0, 0.24, 0.035),
        Color::rgb(7.0, 0.7, 0.08),
        15,
    ));

    let mut scene = RenderScene {
        camera: Camera {
            position: Vec3::new(0.55, 1.52, 5.8),
            target: Vec3::new(0.0, 1.2, -6.5),
            up: Vec3::Y,
            vertical_fov_radians: 58.0_f32.to_radians(),
            near: 0.05,
            far: 60.0,
        },
        materials,
        meshes: Vec::new(),
        lights: vec![
            Light {
                name: "moon_key",
                kind: LightKind::Directional {
                    direction: Vec3::new(-0.28, -0.8, -0.38).normalize_or_zero(),
                },
                color: Color::rgb(0.46, 0.62, 1.0),
                intensity: 1.7,
                casts_shadow: true,
            },
            Light {
                name: "cyan_practical",
                kind: LightKind::Local {
                    position: Vec3::new(-3.8, 2.8, -5.4),
                    radius: 8.0,
                },
                color: Color::rgb(0.1, 0.75, 1.0),
                intensity: 7.0,
                casts_shadow: true,
            },
            Light {
                name: "amber_practical",
                kind: LightKind::Local {
                    position: Vec3::new(3.6, 2.4, -8.0),
                    radius: 7.0,
                },
                color: Color::rgb(1.0, 0.25, 0.04),
                intensity: 6.0,
                casts_shadow: true,
            },
        ],
        sky_color: Color::rgb(0.008, 0.018, 0.045),
        ambient_color: Color::rgb(0.018, 0.03, 0.06),
    };

    scene.meshes.extend([
        plane_mesh(
            "rain_darkened_asphalt",
            7.0,
            27.0,
            0,
            Vec2::new(3.5, 14.0),
            Vec3::new(0.0, -0.06, -5.0),
            true,
        ),
        plane_mesh(
            "left_concrete_walk",
            1.5,
            27.0,
            1,
            Vec2::new(1.5, 13.0),
            Vec3::new(-4.25, 0.0, -5.0),
            true,
        ),
        plane_mesh(
            "right_concrete_walk",
            1.5,
            27.0,
            1,
            Vec2::new(1.5, 13.0),
            Vec3::new(4.25, 0.0, -5.0),
            true,
        ),
        wall_mesh(
            "left_aged_brick",
            27.0,
            7.0,
            2,
            Vec2::new(13.5, 3.5),
            Mat4::rotation_y(90.0_f32.to_radians()) * Mat4::translation(Vec3::new(-5.0, 3.5, -5.0)),
        ),
        wall_mesh(
            "right_aged_brick",
            27.0,
            7.0,
            2,
            Vec2::new(13.5, 3.5),
            Mat4::rotation_y(-90.0_f32.to_radians()) * Mat4::translation(Vec3::new(5.0, 3.5, -5.0)),
        ),
        wall_mesh(
            "alley_end_brick",
            10.0,
            7.0,
            2,
            Vec2::new(5.0, 3.5),
            Mat4::translation(Vec3::new(0.0, 3.5, -18.45)),
        ),
        wall_mesh(
            "left_loading_shutter",
            4.2,
            3.8,
            3,
            Vec2::new(2.0, 2.0),
            Mat4::rotation_y(90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(-4.94, 1.9, -3.8)),
        ),
        wall_mesh(
            "right_loading_shutter",
            3.8,
            3.5,
            3,
            Vec2::new(2.0, 2.0),
            Mat4::rotation_y(-90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(4.94, 1.75, -9.6)),
        ),
        wall_mesh(
            "cyan_wayfinding_light",
            4.0,
            0.34,
            4,
            Vec2::new(1.0, 1.0),
            Mat4::translation(Vec3::new(-1.7, 3.25, -18.38)),
        ),
        wall_mesh(
            "amber_service_light",
            2.6,
            0.28,
            5,
            Vec2::new(1.0, 1.0),
            Mat4::rotation_y(-90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(4.88, 2.65, -6.7)),
        ),
        wall_mesh(
            "cyan_platform_light",
            3.4,
            0.26,
            4,
            Vec2::new(1.0, 1.0),
            Mat4::rotation_y(90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(-4.88, 2.8, -5.4)),
        ),
    ]);

    append_gltf_instances(
        &mut scene,
        root.join("models/Barrel_01/Barrel_01_1k.gltf"),
        "barrel",
        &[
            Mat4::translation(Vec3::new(-3.55, 0.02, -4.3)),
            Mat4::rotation_y(0.35) * Mat4::translation(Vec3::new(-3.0, 0.02, -4.55)),
            Mat4::rotation_y(-0.5) * Mat4::translation(Vec3::new(3.55, 0.02, -12.1)),
        ],
    )?;
    append_gltf_instances(
        &mut scene,
        root.join("models/street_lamp_01/street_lamp_01_1k.gltf"),
        "street_lamp",
        &[
            Mat4::translation(Vec3::new(-4.25, 0.02, 2.2)),
            Mat4::translation(Vec3::new(4.2, 0.02, -4.5)),
            Mat4::translation(Vec3::new(-4.2, 0.02, -11.8)),
        ],
    )?;
    append_gltf_instances(
        &mut scene,
        root.join("models/concrete_road_barrier/concrete_road_barrier_1k.gltf"),
        "barrier",
        &[
            Mat4::rotation_y(-0.22) * Mat4::translation(Vec3::new(0.85, 0.0, -6.2)),
            Mat4::rotation_y(0.18) * Mat4::translation(Vec3::new(0.65, 0.0, -11.3)),
        ],
    )?;
    append_gltf_instances(
        &mut scene,
        root.join("models/steel_frame_shelves_01/steel_frame_shelves_01_1k.gltf"),
        "shelving",
        &[
            Mat4::scale(Vec3::splat(0.1))
                * Mat4::rotation_y(90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(-4.35, 0.02, -3.5)),
            Mat4::scale(Vec3::splat(0.1))
                * Mat4::rotation_y(90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(-4.35, 0.02, -5.0)),
        ],
    )?;
    append_gltf_instances(
        &mut scene,
        root.join("models/utility_box_01/utility_box_01_1k.gltf"),
        "utility_box",
        &[
            Mat4::rotation_y(-90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(4.62, 0.05, -2.1)),
            Mat4::rotation_y(90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(-4.62, 0.05, -13.8)),
        ],
    )?;
    append_gltf_instances(
        &mut scene,
        root.join("models/old_tyre/old_tyre_1k.gltf"),
        "old_tyre",
        &[
            Mat4::rotation_y(0.22) * Mat4::translation(Vec3::new(-3.82, 0.34, -9.1)),
            Mat4::rotation_y(-0.35) * Mat4::translation(Vec3::new(3.78, 0.34, -13.2)),
        ],
    )?;
    append_gltf_instances(
        &mut scene,
        root.join("models/metal_trash_can/metal_trash_can_1k.gltf"),
        "trash_can",
        &[
            Mat4::rotation_y(-0.3) * Mat4::translation(Vec3::new(3.7, 0.02, -3.5)),
            Mat4::rotation_y(0.5) * Mat4::translation(Vec3::new(-3.75, 0.02, -14.2)),
        ],
    )?;

    Ok(scene)
}

fn load_surface_material(
    root: &Path,
    source: &str,
    name: &str,
    class: MaterialClass,
    roughness: f32,
    metallic: f32,
    material_id: u8,
) -> Result<Material, GltfImportError> {
    let directory = root.join("surfaces").join(source);
    let load = |filename: &str| {
        let path = directory.join(filename);
        TextureImage::from_path(&path)
            .map_err(|message| GltfImportError::Gltf(format!("{}: {message}", path.display())))
    };
    Ok(Material {
        name: name.to_string(),
        class,
        base_color: Color::WHITE,
        base_color_texture: Some(load("Diffuse.jpg")?),
        normal_texture: Some(load("nor_gl.jpg")?),
        pbr_texture: Some(load("arm.jpg")?),
        roughness,
        metallic,
        specular_intensity: 0.5,
        cavity: 1.0,
        material_id,
        texture_sample_count: 3,
        flags: MaterialFlags {
            receives_contact_shadows: true,
            uses_bent_normals: true,
            ..MaterialFlags::default()
        },
        emission: Color::BLACK,
    })
}

fn emissive_material(name: &str, base_color: Color, emission: Color, material_id: u8) -> Material {
    Material {
        name: name.to_string(),
        class: MaterialClass::Emissive,
        base_color,
        base_color_texture: None,
        normal_texture: None,
        pbr_texture: None,
        roughness: 0.12,
        metallic: 0.0,
        specular_intensity: 0.2,
        cavity: 1.0,
        material_id,
        texture_sample_count: 1,
        flags: MaterialFlags {
            emissive: true,
            ..MaterialFlags::default()
        },
        emission,
    }
}

fn plane_mesh(
    name: &str,
    width: f32,
    depth: f32,
    material_index: usize,
    uv_scale: Vec2,
    position: Vec3,
    ground: bool,
) -> Mesh {
    let half_width = width * 0.5;
    let half_depth = depth * 0.5;
    let vertex = |x, z, u, v| Vertex {
        position: Vec3::new(x, 0.0, z),
        normal: Vec3::Y,
        tangent: Vec3::X,
        tangent_sign: 1.0,
        uv: Vec2::new(u, v),
        color: Color::WHITE,
    };
    Mesh {
        name: name.to_string(),
        vertices: vec![
            vertex(-half_width, -half_depth, 0.0, 0.0),
            vertex(half_width, -half_depth, uv_scale.x, 0.0),
            vertex(half_width, half_depth, uv_scale.x, uv_scale.y),
            vertex(-half_width, half_depth, 0.0, uv_scale.y),
        ],
        indices: vec![[0, 2, 1], [0, 3, 2]],
        material_index,
        transform: Mat4::translation(position),
        shadow_class: ShadowCasterClass::None,
        is_floor_or_ground: ground,
        prepass_eligible: true,
        reflection_proxy: false,
    }
}

fn wall_mesh(
    name: &str,
    width: f32,
    height: f32,
    material_index: usize,
    uv_scale: Vec2,
    transform: Mat4,
) -> Mesh {
    let mut vertices = upright_quad_vertices(width, height, Color::WHITE);
    vertices[1].uv.x = uv_scale.x;
    vertices[2].uv = uv_scale;
    vertices[3].uv.y = uv_scale.y;
    Mesh {
        name: name.to_string(),
        vertices,
        indices: vec![[0, 1, 2], [0, 2, 3]],
        material_index,
        transform,
        shadow_class: ShadowCasterClass::Main,
        is_floor_or_ground: false,
        prepass_eligible: true,
        reflection_proxy: false,
    }
}

fn append_gltf_instances(
    scene: &mut RenderScene,
    path: impl AsRef<Path>,
    prefix: &str,
    transforms: &[Mat4],
) -> Result<(), GltfImportError> {
    let material_offset = scene.materials.len();
    let (mut materials, meshes) = import_gltf(path)?;
    for (index, material) in materials.iter_mut().enumerate() {
        material.material_id = (material_offset + index).min(255) as u8;
    }
    scene.materials.extend(materials);
    for (instance, placement) in transforms.iter().enumerate() {
        for source in &meshes {
            let mut mesh = source.clone();
            mesh.name = format!("{prefix}_{instance}_{}", source.name);
            mesh.material_index += material_offset;
            mesh.transform = mesh.transform * *placement;
            scene.meshes.push(mesh);
        }
    }
    Ok(())
}

fn quad_vertices(size: f32, normal: Vec3, y: f32, color: Color) -> Vec<Vertex> {
    let s = size;
    vec![
        Vertex {
            position: Vec3::new(-s, y, -s),
            normal,
            tangent: Vec3::X,
            tangent_sign: 1.0,
            uv: Vec2::new(0.0, 0.0),
            color,
        },
        Vertex {
            position: Vec3::new(s, y, -s),
            normal,
            tangent: Vec3::X,
            tangent_sign: 1.0,
            uv: Vec2::new(1.0, 0.0),
            color,
        },
        Vertex {
            position: Vec3::new(s, y, s),
            normal,
            tangent: Vec3::X,
            tangent_sign: 1.0,
            uv: Vec2::new(1.0, 1.0),
            color,
        },
        Vertex {
            position: Vec3::new(-s, y, s),
            normal,
            tangent: Vec3::X,
            tangent_sign: 1.0,
            uv: Vec2::new(0.0, 1.0),
            color,
        },
    ]
}

fn upright_quad_vertices(width: f32, height: f32, color: Color) -> Vec<Vertex> {
    let hw = width * 0.5;
    let hh = height * 0.5;
    let normal = Vec3::new(0.0, 0.0, 1.0);
    vec![
        Vertex {
            position: Vec3::new(-hw, -hh, 0.0),
            normal,
            tangent: Vec3::X,
            tangent_sign: 1.0,
            uv: Vec2::new(0.0, 0.0),
            color,
        },
        Vertex {
            position: Vec3::new(hw, -hh, 0.0),
            normal,
            tangent: Vec3::X,
            tangent_sign: 1.0,
            uv: Vec2::new(1.0, 0.0),
            color,
        },
        Vertex {
            position: Vec3::new(hw, hh, 0.0),
            normal,
            tangent: Vec3::X,
            tangent_sign: 1.0,
            uv: Vec2::new(1.0, 1.0),
            color,
        },
        Vertex {
            position: Vec3::new(-hw, hh, 0.0),
            normal,
            tangent: Vec3::X,
            tangent_sign: 1.0,
            uv: Vec2::new(0.0, 1.0),
            color,
        },
    ]
}
