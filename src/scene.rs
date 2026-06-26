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
    Directional {
        direction: Vec3,
    },
    Local {
        position: Vec3,
        radius: f32,
    },
    Spot {
        position: Vec3,
        direction: Vec3,
        radius: f32,
        inner_angle_radians: f32,
        outer_angle_radians: f32,
    },
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

pub fn build_dust2_scene(root: impl AsRef<Path>) -> Result<RenderScene, GltfImportError> {
    let root = root.as_ref();
    let world_path = root
        .join("generated")
        .join("cs2")
        .join("maps")
        .join("de_dust2")
        .join("world_physics_physics.glb");
    if !world_path.is_file() {
        return Err(GltfImportError::Gltf(format!(
            "missing local CS2 Dust2 export at {}; run scripts/import_cs2_dust2.sh with SOURCE2VIEWER_CLI pointing at ValveResourceFormat's Source2Viewer-CLI",
            world_path.display()
        )));
    }

    let (_, mut meshes) = import_gltf(&world_path)?;
    let materials = cs2_dust2_materials();
    for mesh in &mut meshes {
        let source_name = mesh.name.clone();
        mesh.name = format!("cs2_dust2_{source_name}");
        mesh.material_index = cs2_dust2_material_index(&source_name);
        mesh.shadow_class = ShadowCasterClass::None;
        mesh.is_floor_or_ground = cs2_dust2_ground_mesh(&source_name);
        mesh.prepass_eligible = false;
        mesh.reflection_proxy = false;
    }

    Ok(RenderScene {
        camera: Camera {
            position: Vec3::new(50.0, 1.8, -45.0),
            target: Vec3::new(30.0, 1.8, -30.0),
            up: Vec3::Y,
            vertical_fov_radians: 62.0_f32.to_radians(),
            near: 0.05,
            far: 620.0,
        },
        materials,
        meshes,
        lights: vec![
            Light {
                name: "cs2_dust2_desert_sun",
                kind: LightKind::Directional {
                    direction: Vec3::new(-0.42, -0.86, -0.30).normalize_or_zero(),
                },
                color: Color::rgb(1.0, 0.93, 0.78),
                intensity: 2.8,
                casts_shadow: true,
            },
            Light {
                name: "cs2_dust2_mid_bounce",
                kind: LightKind::Local {
                    position: Vec3::new(14.0, 8.0, -8.0),
                    radius: 88.0,
                },
                color: Color::rgb(1.0, 0.62, 0.34),
                intensity: 2.2,
                casts_shadow: false,
            },
            Light {
                name: "cs2_dust2_tunnel_fill",
                kind: LightKind::Local {
                    position: Vec3::new(-38.0, 6.0, -24.0),
                    radius: 34.0,
                },
                color: Color::rgb(1.0, 0.54, 0.28),
                intensity: 1.5,
                casts_shadow: false,
            },
        ],
        sky_color: Color::rgb(0.50, 0.64, 0.84),
        ambient_color: Color::rgb(0.22, 0.18, 0.12),
    })
}

fn cs2_dust2_materials() -> Vec<Material> {
    vec![
        solid_material(
            "cs2_dust2_concrete_plaster",
            SolidMaterialParams {
                class: MaterialClass::Concrete,
                base_color: Color::rgb(0.70, 0.58, 0.42),
                roughness: 0.86,
                metallic: 0.0,
                specular_intensity: 0.28,
                cavity: 1.0,
                material_id: 40,
            },
        ),
        solid_material(
            "cs2_dust2_dirt_sand",
            SolidMaterialParams {
                class: MaterialClass::Concrete,
                base_color: Color::rgb(0.62, 0.48, 0.31),
                roughness: 0.95,
                metallic: 0.0,
                specular_intensity: 0.12,
                cavity: 1.0,
                material_id: 41,
            },
        ),
        solid_material(
            "cs2_dust2_wood",
            SolidMaterialParams {
                class: MaterialClass::Concrete,
                base_color: Color::rgb(0.48, 0.32, 0.20),
                roughness: 0.78,
                metallic: 0.0,
                specular_intensity: 0.24,
                cavity: 1.0,
                material_id: 42,
            },
        ),
        solid_material(
            "cs2_dust2_metal",
            SolidMaterialParams {
                class: MaterialClass::Metal,
                base_color: Color::rgb(0.42, 0.38, 0.33),
                roughness: 0.56,
                metallic: 1.0,
                specular_intensity: 0.62,
                cavity: 1.0,
                material_id: 43,
            },
        ),
        solid_material(
            "cs2_dust2_chainlink",
            SolidMaterialParams {
                class: MaterialClass::Metal,
                base_color: Color::rgb(0.30, 0.32, 0.30),
                roughness: 0.48,
                metallic: 1.0,
                specular_intensity: 0.54,
                cavity: 1.0,
                material_id: 44,
            },
        ),
        solid_material(
            "cs2_dust2_glass",
            SolidMaterialParams {
                class: MaterialClass::Glass,
                base_color: Color::rgb(0.46, 0.58, 0.66),
                roughness: 0.08,
                metallic: 0.0,
                specular_intensity: 0.75,
                cavity: 1.0,
                material_id: 45,
            },
        ),
        solid_material(
            "cs2_dust2_rock",
            SolidMaterialParams {
                class: MaterialClass::Concrete,
                base_color: Color::rgb(0.50, 0.42, 0.32),
                roughness: 0.90,
                metallic: 0.0,
                specular_intensity: 0.18,
                cavity: 1.0,
                material_id: 46,
            },
        ),
        solid_material(
            "cs2_dust2_rubber_plastic",
            SolidMaterialParams {
                class: MaterialClass::Default,
                base_color: Color::rgb(0.08, 0.08, 0.07),
                roughness: 0.82,
                metallic: 0.0,
                specular_intensity: 0.20,
                cavity: 1.0,
                material_id: 47,
            },
        ),
        solid_material(
            "cs2_dust2_default",
            SolidMaterialParams {
                class: MaterialClass::Concrete,
                base_color: Color::rgb(0.64, 0.52, 0.38),
                roughness: 0.88,
                metallic: 0.0,
                specular_intensity: 0.20,
                cavity: 1.0,
                material_id: 48,
            },
        ),
    ]
}

fn cs2_dust2_material_index(name: &str) -> usize {
    if name.contains("chainlink") {
        4
    } else if name.contains("glass") {
        5
    } else if name.contains("metal")
        || name.contains("barrel")
        || name.contains("dumpster")
        || name.contains("vehicle")
    {
        3
    } else if name.contains("wood") {
        2
    } else if name.contains("dirt") || name.contains("sand") {
        1
    } else if name.contains("rock") {
        6
    } else if name.contains("rubber") || name.contains("plastic") {
        7
    } else if name.contains("concrete") {
        0
    } else {
        8
    }
}

fn cs2_dust2_ground_mesh(name: &str) -> bool {
    name.contains("dirt")
        || name.contains("sand")
        || name.contains("concrete")
        || name.contains("carpet")
        || name.contains("rock")
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
    let dark_fixture_material = materials.len();
    materials.push(solid_material(
        "charcoal_fixture_metal",
        SolidMaterialParams {
            class: MaterialClass::Metal,
            base_color: Color::rgb(0.030, 0.036, 0.042),
            roughness: 0.34,
            metallic: 0.88,
            specular_intensity: 0.72,
            cavity: 0.72,
            material_id: 16,
        },
    ));
    let warm_edge_material = materials.len();
    materials.push(solid_material(
        "aged_brass_edge_trim",
        SolidMaterialParams {
            class: MaterialClass::Metal,
            base_color: Color::rgb(0.55, 0.43, 0.26),
            roughness: 0.30,
            metallic: 0.78,
            specular_intensity: 0.80,
            cavity: 0.82,
            material_id: 17,
        },
    ));
    let puddle_material = materials.len();
    materials.push(solid_material(
        "foreground_clear_puddle",
        SolidMaterialParams {
            class: MaterialClass::Glass,
            base_color: Color::new(0.026, 0.034, 0.042, 0.18),
            roughness: 0.038,
            metallic: 0.0,
            specular_intensity: 1.0,
            cavity: 1.0,
            material_id: 18,
        },
    ));
    {
        let load_puddle_texture = |filename: &str| {
            let path = root.join("puddles").join(filename);
            TextureImage::from_path(&path)
                .map_err(|message| GltfImportError::Gltf(format!("{}: {message}", path.display())))
        };
        let puddle = &mut materials[puddle_material];
        puddle.normal_texture = Some(load_puddle_texture("generated_puddle_normal.png")?);
        puddle.pbr_texture = Some(load_puddle_texture("generated_puddle_arm.png")?);
        puddle.texture_sample_count = 3;
    }
    let cyan_generated_panel_material = materials.len();
    materials.push(textured_generated_material(
        "generated_cyan_transit_panel",
        load_generated_texture(root, "cyan_transit_panel.png")?,
        TexturedGeneratedMaterialParams {
            class: MaterialClass::Emissive,
            base_color: Color::rgb(0.88, 0.96, 1.0),
            roughness: 0.18,
            metallic: 0.0,
            specular_intensity: 0.22,
            cavity: 1.0,
            material_id: 19,
            emission: Color::rgb(0.18, 2.4, 3.3),
        },
    ));
    let amber_generated_panel_material = materials.len();
    materials.push(textured_generated_material(
        "generated_amber_hazard_placard",
        load_generated_texture(root, "amber_hazard_placard.png")?,
        TexturedGeneratedMaterialParams {
            class: MaterialClass::Emissive,
            base_color: Color::rgb(1.0, 0.90, 0.72),
            roughness: 0.24,
            metallic: 0.0,
            specular_intensity: 0.18,
            cavity: 0.95,
            material_id: 20,
            emission: Color::rgb(2.8, 0.82, 0.12),
        },
    ));
    let wet_tarp_material = materials.len();
    materials.push(textured_generated_material(
        "generated_wet_tarp_cable_net",
        load_generated_texture(root, "wet_tarp_cable_net.png")?,
        TexturedGeneratedMaterialParams {
            class: MaterialClass::Default,
            base_color: Color::rgb(0.72, 0.78, 0.88),
            roughness: 0.58,
            metallic: 0.0,
            specular_intensity: 0.34,
            cavity: 0.86,
            material_id: 21,
            emission: Color::BLACK,
        },
    ));
    let wet_corrugated_metal_material = materials.len();
    materials.push(generated_texture_set_material(
        root,
        "generated_wet_corrugated_metal",
        "wet_corrugated_metal",
        GeneratedTextureSetMaterialParams {
            class: MaterialClass::Metal,
            base_color: Color::rgb(0.90, 0.96, 1.0),
            roughness: 0.58,
            metallic: 0.86,
            specular_intensity: 0.72,
            cavity: 0.92,
            material_id: 22,
        },
    )?);
    let oily_patched_concrete_material = materials.len();
    materials.push(generated_texture_set_material(
        root,
        "generated_oily_patched_concrete",
        "oily_patched_concrete",
        GeneratedTextureSetMaterialParams {
            class: MaterialClass::Concrete,
            base_color: Color::rgb(0.92, 0.94, 0.96),
            roughness: 0.86,
            metallic: 0.0,
            specular_intensity: 0.36,
            cavity: 0.88,
            material_id: 23,
        },
    )?);
    let city_shopfront_material = materials.len();
    materials.push(textured_generated_material(
        "generated_city_shopfront_facade",
        load_generated_texture(root, "city_shopfront_facade.png")?,
        TexturedGeneratedMaterialParams {
            class: MaterialClass::Default,
            base_color: Color::rgb(0.86, 0.91, 0.94),
            roughness: 0.54,
            metallic: 0.0,
            specular_intensity: 0.30,
            cavity: 0.86,
            material_id: 24,
            emission: Color::BLACK,
        },
    ));
    let city_skyline_material = materials.len();
    materials.push(textured_generated_material(
        "generated_city_apartment_skyline",
        load_generated_texture(root, "city_apartment_skyline.png")?,
        TexturedGeneratedMaterialParams {
            class: MaterialClass::Emissive,
            base_color: Color::rgb(0.70, 0.82, 0.94),
            roughness: 0.66,
            metallic: 0.0,
            specular_intensity: 0.20,
            cavity: 0.92,
            material_id: 25,
            emission: Color::rgb(0.16, 0.30, 0.48),
        },
    ));
    let market_awning_material = materials.len();
    materials.push(textured_generated_material(
        "generated_monsoon_market_awning",
        load_generated_texture(root, "monsoon_market_awning.png")?,
        TexturedGeneratedMaterialParams {
            class: MaterialClass::Default,
            base_color: Color::rgb(0.92, 0.88, 0.78),
            roughness: 0.62,
            metallic: 0.0,
            specular_intensity: 0.38,
            cavity: 0.88,
            material_id: 26,
            emission: Color::BLACK,
        },
    ));
    let rain_black_trim_material = materials.len();
    materials.push(solid_material(
        "rain_black_object_trim",
        SolidMaterialParams {
            class: MaterialClass::Default,
            base_color: Color::rgb(0.035, 0.040, 0.045),
            roughness: 0.55,
            metallic: 0.0,
            specular_intensity: 0.35,
            cavity: 0.82,
            material_id: 27,
        },
    ));

    let mut scene = RenderScene {
        camera: Camera {
            position: Vec3::new(0.35, 3.05, -30.5),
            target: Vec3::new(0.0, 2.42, -61.5),
            up: Vec3::Y,
            vertical_fov_radians: 78.0_f32.to_radians(),
            near: 0.05,
            far: 180.0,
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
                kind: LightKind::Spot {
                    position: Vec3::new(-3.8, 2.8, -5.4),
                    direction: Vec3::new(0.92, -0.30, -0.25).normalize_or_zero(),
                    radius: 8.5,
                    inner_angle_radians: 46.0_f32.to_radians(),
                    outer_angle_radians: 67.0_f32.to_radians(),
                },
                color: Color::rgb(0.1, 0.75, 1.0),
                intensity: 7.6,
                casts_shadow: true,
            },
            Light {
                name: "amber_practical",
                kind: LightKind::Spot {
                    position: Vec3::new(3.6, 2.4, -8.0),
                    direction: Vec3::new(-0.94, -0.24, 0.15).normalize_or_zero(),
                    radius: 8.2,
                    inner_angle_radians: 44.0_f32.to_radians(),
                    outer_angle_radians: 67.0_f32.to_radians(),
                },
                color: Color::rgb(1.0, 0.25, 0.04),
                intensity: 7.2,
                casts_shadow: true,
            },
            Light {
                name: "right_marker_fill",
                kind: LightKind::Local {
                    position: Vec3::new(4.08, 2.85, -3.25),
                    radius: 5.8,
                },
                color: Color::rgb(1.0, 0.32, 0.08),
                intensity: 4.8,
                casts_shadow: false,
            },
            Light {
                name: "right_barrel_read_fill",
                kind: LightKind::Local {
                    position: Vec3::new(3.12, 0.92, -3.35),
                    radius: 3.65,
                },
                color: Color::rgb(1.0, 0.42, 0.16),
                intensity: 2.25,
                casts_shadow: false,
            },
            Light {
                name: "left_barrel_read_fill",
                kind: LightKind::Local {
                    position: Vec3::new(-3.18, 1.02, -4.35),
                    radius: 3.45,
                },
                color: Color::rgb(0.18, 0.66, 1.0),
                intensity: 1.55,
                casts_shadow: false,
            },
            Light {
                name: "right_far_marker_fill",
                kind: LightKind::Local {
                    position: Vec3::new(4.05, 3.10, -12.9),
                    radius: 5.0,
                },
                color: Color::rgb(1.0, 0.30, 0.07),
                intensity: 3.6,
                casts_shadow: false,
            },
            Light {
                name: "deep_cyan_panel_fill",
                kind: LightKind::Local {
                    position: Vec3::new(1.8, 3.15, -24.1),
                    radius: 6.2,
                },
                color: Color::rgb(0.12, 0.78, 1.0),
                intensity: 4.6,
                casts_shadow: false,
            },
            Light {
                name: "deep_amber_service_fill",
                kind: LightKind::Local {
                    position: Vec3::new(-3.0, 2.25, -20.2),
                    radius: 5.3,
                },
                color: Color::rgb(1.0, 0.42, 0.10),
                intensity: 3.2,
                casts_shadow: false,
            },
            Light {
                name: "left_tarp_graze_fill",
                kind: LightKind::Local {
                    position: Vec3::new(-3.75, 2.85, -12.6),
                    radius: 4.8,
                },
                color: Color::rgb(0.34, 0.64, 1.0),
                intensity: 2.1,
                casts_shadow: false,
            },
            Light {
                name: "city_cross_street_cyan",
                kind: LightKind::Local {
                    position: Vec3::new(-8.8, 3.4, -36.0),
                    radius: 12.5,
                },
                color: Color::rgb(0.12, 0.72, 1.0),
                intensity: 4.4,
                casts_shadow: false,
            },
            Light {
                name: "market_lane_amber",
                kind: LightKind::Local {
                    position: Vec3::new(7.6, 2.9, -44.5),
                    radius: 13.0,
                },
                color: Color::rgb(1.0, 0.48, 0.14),
                intensity: 4.1,
                casts_shadow: false,
            },
            Light {
                name: "distant_apartment_window_glow",
                kind: LightKind::Local {
                    position: Vec3::new(0.0, 8.2, -72.0),
                    radius: 28.0,
                },
                color: Color::rgb(0.42, 0.68, 1.0),
                intensity: 2.6,
                casts_shadow: false,
            },
        ],
        sky_color: Color::rgb(0.006, 0.014, 0.034),
        ambient_color: Color::rgb(0.024, 0.030, 0.046),
    };

    scene.meshes.extend([
        plane_mesh(
            "rain_darkened_asphalt",
            13.5,
            96.0,
            0,
            Vec2::new(6.75, 48.0),
            Vec3::new(0.0, -0.06, -30.0),
            true,
        ),
        plane_mesh(
            "left_concrete_walk",
            2.8,
            96.0,
            1,
            Vec2::new(2.8, 48.0),
            Vec3::new(-8.15, 0.0, -30.0),
            true,
        ),
        plane_mesh(
            "right_concrete_walk",
            2.8,
            96.0,
            1,
            Vec2::new(2.8, 48.0),
            Vec3::new(8.15, 0.0, -30.0),
            true,
        ),
        wall_mesh(
            "left_aged_brick",
            36.0,
            7.0,
            2,
            Vec2::new(18.0, 3.5),
            Mat4::rotation_y(90.0_f32.to_radians()) * Mat4::translation(Vec3::new(-5.0, 3.5, -7.5)),
        ),
        wall_mesh(
            "right_aged_brick",
            36.0,
            7.0,
            2,
            Vec2::new(18.0, 3.5),
            Mat4::rotation_y(-90.0_f32.to_radians()) * Mat4::translation(Vec3::new(5.0, 3.5, -7.5)),
        ),
        wall_mesh(
            "alley_end_brick",
            36.0,
            18.0,
            2,
            Vec2::new(12.0, 6.0),
            Mat4::translation(Vec3::new(0.0, 8.6, -78.5)),
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
        wall_mesh(
            "left_recessed_service_bay",
            3.2,
            4.6,
            3,
            Vec2::new(1.5, 2.3),
            Mat4::rotation_y(90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(-4.78, 2.28, -7.25)),
        ),
        wall_mesh(
            "right_stacked_service_bay",
            2.4,
            4.2,
            3,
            Vec2::new(1.2, 2.1),
            Mat4::rotation_y(-90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(4.78, 2.1, -4.75)),
        ),
        wall_mesh(
            "mid_alley_overhead_duct",
            8.7,
            0.42,
            3,
            Vec2::new(3.0, 0.5),
            Mat4::translation(Vec3::new(0.0, 4.95, -8.4)),
        ),
        wall_mesh(
            "left_wall_cable_tray",
            7.6,
            0.18,
            3,
            Vec2::new(3.0, 0.35),
            Mat4::rotation_y(90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(-4.72, 4.45, -10.4)),
        ),
        wall_mesh(
            "right_wall_steam_riser",
            0.42,
            5.4,
            3,
            Vec2::new(0.4, 2.7),
            Mat4::rotation_y(-90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(4.72, 2.72, -12.8)),
        ),
        wall_mesh(
            "hanging_wayfinding_panel",
            1.2,
            1.05,
            5,
            Vec2::new(1.0, 1.0),
            Mat4::rotation_y(12.0_f32.to_radians())
                * Mat4::translation(Vec3::new(-1.95, 3.05, -8.7)),
        ),
        wall_mesh(
            "mid_alley_shadow_crossbeam",
            7.8,
            0.55,
            3,
            Vec2::new(3.2, 0.55),
            Mat4::translation(Vec3::new(0.0, 4.18, -6.7)),
        ),
        wall_mesh(
            "left_pipe_bundle_upper",
            0.72,
            4.8,
            3,
            Vec2::new(0.6, 2.4),
            Mat4::rotation_y(90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(-4.66, 2.55, -6.35)),
        ),
        wall_mesh(
            "right_shadow_box_awning",
            2.8,
            0.85,
            3,
            Vec2::new(1.6, 0.7),
            Mat4::rotation_y(-90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(4.66, 3.25, -8.15)),
        ),
        wall_mesh(
            "center_dangling_cable_sheet",
            2.2,
            1.4,
            3,
            Vec2::new(1.0, 1.2),
            Mat4::rotation_y(8.0_f32.to_radians()) * Mat4::translation(Vec3::new(0.85, 3.1, -10.8)),
        ),
        wall_mesh(
            "far_scaffold_shadow",
            5.2,
            3.2,
            3,
            Vec2::new(2.5, 1.6),
            Mat4::translation(Vec3::new(0.0, 2.65, -14.8)),
        ),
    ]);

    scene.meshes.extend([
        detail_wall_mesh(
            "cyan_platform_light_backplate",
            3.9,
            0.68,
            dark_fixture_material,
            Vec2::new(1.0, 1.0),
            Mat4::rotation_y(90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(-4.90, 2.80, -5.4)),
        ),
        detail_wall_mesh(
            "cyan_platform_light_lower_lip",
            3.8,
            0.08,
            warm_edge_material,
            Vec2::new(1.0, 1.0),
            Mat4::rotation_y(90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(-4.86, 2.47, -5.4)),
        ),
        detail_wall_mesh(
            "amber_service_light_backplate",
            3.0,
            0.62,
            dark_fixture_material,
            Vec2::new(1.0, 1.0),
            Mat4::rotation_y(-90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(4.90, 2.65, -6.7)),
        ),
        detail_wall_mesh(
            "amber_service_light_upper_lip",
            2.8,
            0.08,
            warm_edge_material,
            Vec2::new(1.0, 1.0),
            Mat4::rotation_y(-90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(4.86, 2.98, -6.7)),
        ),
        detail_wall_mesh(
            "far_cyan_wayfinding_backplate",
            4.4,
            0.74,
            dark_fixture_material,
            Vec2::new(1.0, 1.0),
            Mat4::translation(Vec3::new(-1.7, 3.25, -18.40)),
        ),
        detail_wall_mesh(
            "left_loading_shutter_top_trim",
            4.5,
            0.12,
            warm_edge_material,
            Vec2::new(1.0, 1.0),
            Mat4::rotation_y(90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(-4.86, 3.88, -3.8)),
        ),
        detail_wall_mesh(
            "left_loading_shutter_side_trim",
            0.12,
            3.9,
            warm_edge_material,
            Vec2::new(1.0, 1.0),
            Mat4::rotation_y(90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(-4.84, 1.93, -1.62)),
        ),
        detail_wall_mesh(
            "right_loading_shutter_top_trim",
            4.1,
            0.12,
            warm_edge_material,
            Vec2::new(1.0, 1.0),
            Mat4::rotation_y(-90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(4.86, 3.55, -9.6)),
        ),
        detail_wall_mesh(
            "right_wall_foreground_kickplate",
            15.5,
            0.30,
            dark_fixture_material,
            Vec2::new(5.0, 1.0),
            Mat4::rotation_y(-90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(4.58, 0.34, -4.8)),
        ),
        detail_wall_mesh(
            "right_wall_near_access_panel",
            2.7,
            3.3,
            dark_fixture_material,
            Vec2::new(1.0, 1.6),
            Mat4::rotation_y(-90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(4.54, 2.15, -2.35)),
        ),
        detail_wall_mesh(
            "right_wall_mid_access_panel",
            2.2,
            2.5,
            dark_fixture_material,
            Vec2::new(1.0, 1.2),
            Mat4::rotation_y(-90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(4.54, 2.05, -11.65)),
        ),
        detail_wall_mesh(
            "right_wall_conduit_upper",
            9.5,
            0.10,
            warm_edge_material,
            Vec2::new(4.0, 1.0),
            Mat4::rotation_y(-90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(4.50, 3.72, -5.2)),
        ),
        detail_wall_mesh(
            "right_wall_conduit_middle",
            8.8,
            0.08,
            warm_edge_material,
            Vec2::new(4.0, 1.0),
            Mat4::rotation_y(-90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(4.50, 2.62, -4.7)),
        ),
        detail_wall_mesh(
            "right_wall_conduit_lower",
            7.6,
            0.08,
            warm_edge_material,
            Vec2::new(4.0, 1.0),
            Mat4::rotation_y(-90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(4.50, 1.42, -5.6)),
        ),
        detail_wall_mesh(
            "right_wall_vertical_rib_near",
            0.12,
            5.0,
            dark_fixture_material,
            Vec2::new(1.0, 2.0),
            Mat4::rotation_y(-90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(4.48, 2.55, -0.85)),
        ),
        detail_wall_mesh(
            "right_wall_vertical_rib_mid",
            0.12,
            4.7,
            dark_fixture_material,
            Vec2::new(1.0, 2.0),
            Mat4::rotation_y(-90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(4.48, 2.42, -6.95)),
        ),
        detail_wall_mesh(
            "right_wall_small_amber_marker",
            0.72,
            0.14,
            5,
            Vec2::new(1.0, 1.0),
            Mat4::rotation_y(-90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(4.46, 2.92, -3.2)),
        ),
        detail_wall_mesh(
            "right_wall_far_amber_marker",
            0.58,
            0.12,
            5,
            Vec2::new(1.0, 1.0),
            Mat4::rotation_y(-90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(4.46, 3.25, -12.9)),
        ),
        box_mesh(
            "left_generated_cyan_transit_panel",
            Vec3::new(3.85, 2.28, 0.16),
            cyan_generated_panel_material,
            Vec2::new(1.0, 1.0),
            Mat4::rotation_y(90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(-4.47, 2.85, -16.55)),
            false,
            false,
        ),
        box_mesh(
            "right_generated_amber_hazard_placard",
            Vec3::new(2.75, 1.84, 0.16),
            amber_generated_panel_material,
            Vec2::new(1.0, 1.0),
            Mat4::rotation_y(-90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(4.46, 2.35, -16.15)),
            false,
            false,
        ),
        box_mesh(
            "deep_generated_cyan_transit_panel",
            Vec3::new(4.7, 2.82, 0.16),
            cyan_generated_panel_material,
            Vec2::new(1.0, 1.0),
            Mat4::translation(Vec3::new(1.75, 3.0, -25.30)),
            false,
            false,
        ),
        box_mesh(
            "deep_generated_amber_service_marker",
            Vec3::new(2.1, 1.34, 0.16),
            amber_generated_panel_material,
            Vec2::new(1.0, 1.0),
            Mat4::translation(Vec3::new(-3.15, 2.25, -25.28)),
            false,
            false,
        ),
        box_mesh(
            "left_wet_tarp_cable_net",
            Vec3::new(4.45, 3.35, 0.18),
            wet_tarp_material,
            Vec2::new(1.0, 1.0),
            Mat4::rotation_y(90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(-4.50, 2.35, -11.45)),
            false,
            false,
        ),
        box_mesh(
            "center_hanging_tarp_cable_sheet",
            Vec3::new(3.55, 2.6, 0.18),
            wet_tarp_material,
            Vec2::new(1.0, 1.0),
            Mat4::rotation_y(-8.0_f32.to_radians())
                * Mat4::translation(Vec3::new(-0.78, 2.55, -18.10)),
            false,
            false,
        ),
        box_mesh(
            "right_far_tarp_patch",
            Vec3::new(3.25, 2.45, 0.18),
            wet_tarp_material,
            Vec2::new(1.0, 1.0),
            Mat4::rotation_y(-90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(4.50, 2.15, -20.45)),
            false,
            false,
        ),
        box_mesh(
            "right_generated_corrugated_service_skin",
            Vec3::new(3.6, 4.9, 0.20),
            wet_corrugated_metal_material,
            Vec2::new(1.0, 1.45),
            Mat4::rotation_y(-90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(4.42, 2.55, -14.35)),
            false,
            true,
        ),
        box_mesh(
            "left_generated_corrugated_access_panel",
            Vec3::new(2.85, 3.95, 0.20),
            wet_corrugated_metal_material,
            Vec2::new(0.85, 1.25),
            Mat4::rotation_y(90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(-4.42, 2.12, -18.95)),
            false,
            true,
        ),
        box_mesh(
            "deep_generated_corrugated_rollup_door",
            Vec3::new(3.35, 4.25, 0.20),
            wet_corrugated_metal_material,
            Vec2::new(1.0, 1.25),
            Mat4::translation(Vec3::new(-2.10, 2.20, -25.24)),
            false,
            true,
        ),
        detail_wall_mesh(
            "deep_overhead_service_bridge",
            8.6,
            0.62,
            dark_fixture_material,
            Vec2::new(3.5, 1.0),
            Mat4::translation(Vec3::new(0.0, 4.46, -17.55)),
        ),
        detail_wall_mesh(
            "deep_overhead_cable_bundle",
            8.8,
            0.16,
            warm_edge_material,
            Vec2::new(4.0, 1.0),
            Mat4::translation(Vec3::new(0.0, 4.12, -19.30)),
        ),
        detail_wall_mesh(
            "far_left_stacked_service_panels",
            2.75,
            3.1,
            dark_fixture_material,
            Vec2::new(1.0, 1.6),
            Mat4::rotation_y(90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(-4.54, 2.05, -21.40)),
        ),
        detail_wall_mesh(
            "far_right_vertical_rib",
            0.14,
            5.3,
            dark_fixture_material,
            Vec2::new(1.0, 2.0),
            Mat4::rotation_y(-90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(4.48, 2.66, -22.65)),
        ),
        puddle_mesh(
            "foreground_reflection_puddle",
            2.05,
            3.72,
            puddle_material,
            Vec2::new(1.15, 2.2),
            Vec3::new(0.22, -0.049, 0.95),
        ),
        puddle_mesh(
            "left_cyan_reflection_slick",
            1.05,
            5.80,
            puddle_material,
            Vec2::new(0.9, 2.9),
            Vec3::new(-1.82, -0.048, -3.65),
        ),
        puddle_mesh(
            "mid_alley_reflection_puddle",
            2.25,
            3.85,
            puddle_material,
            Vec2::new(1.1, 1.8),
            Vec3::new(1.20, -0.048, -7.05),
        ),
        plane_mesh(
            "generated_oily_concrete_foreground_slab",
            2.45,
            4.80,
            oily_patched_concrete_material,
            Vec2::new(0.95, 1.85),
            Vec3::new(-1.55, -0.045, 1.30),
            true,
        ),
        plane_mesh(
            "generated_oily_concrete_mid_service_slab",
            2.25,
            5.40,
            oily_patched_concrete_material,
            Vec2::new(0.85, 2.10),
            Vec3::new(-2.25, -0.044, -12.45),
            true,
        ),
        plane_mesh(
            "generated_oily_concrete_far_gate_slab",
            3.10,
            3.70,
            oily_patched_concrete_material,
            Vec2::new(1.2, 1.45),
            Vec3::new(2.05, -0.044, -22.25),
            true,
        ),
        puddle_mesh(
            "right_amber_service_reflection_slick",
            1.45,
            4.95,
            puddle_material,
            Vec2::new(0.8, 2.5),
            Vec3::new(2.18, -0.048, -13.70),
        ),
        puddle_mesh(
            "deep_cyan_panel_reflection_pool",
            2.65,
            4.4,
            puddle_material,
            Vec2::new(1.35, 2.25),
            Vec3::new(0.88, -0.048, -20.65),
        ),
        puddle_mesh(
            "far_service_gate_rain_pool",
            1.85,
            2.75,
            puddle_material,
            Vec2::new(0.9, 1.5),
            Vec3::new(-2.45, -0.048, -22.95),
        ),
    ]);

    scene.meshes.extend([
        plane_mesh(
            "city_cross_street_wet_asphalt",
            38.0,
            14.0,
            0,
            Vec2::new(19.0, 7.0),
            Vec3::new(0.0, -0.055, -38.5),
            true,
        ),
        plane_mesh(
            "left_market_lane_wet_pavers",
            12.0,
            38.0,
            1,
            Vec2::new(6.0, 19.0),
            Vec3::new(-14.2, -0.052, -45.5),
            true,
        ),
        plane_mesh(
            "right_bazaar_lane_wet_pavers",
            12.0,
            38.0,
            1,
            Vec2::new(6.0, 19.0),
            Vec3::new(14.2, -0.052, -45.5),
            true,
        ),
    ]);

    append_city_shopfront_row(
        &mut scene,
        "left_city_shopfront_row_near",
        CityShopfrontRow {
            facing: FacadeFacing::Left,
            face_base: Vec3::new(-11.8, 0.0, -33.8),
            length: 10.5,
            height: 7.8,
            bays: 3,
        },
        CityShopfrontMaterials {
            shell: oily_patched_concrete_material,
            panel: city_shopfront_material,
            awning: market_awning_material,
            trim: rain_black_trim_material,
            cool_light: 4,
            warm_light: 5,
        },
    );
    append_city_shopfront_row(
        &mut scene,
        "left_city_shopfront_row_far",
        CityShopfrontRow {
            facing: FacadeFacing::Left,
            face_base: Vec3::new(-13.9, 0.0, -52.4),
            length: 12.5,
            height: 8.4,
            bays: 4,
        },
        CityShopfrontMaterials {
            shell: oily_patched_concrete_material,
            panel: city_shopfront_material,
            awning: market_awning_material,
            trim: rain_black_trim_material,
            cool_light: 4,
            warm_light: 5,
        },
    );
    append_city_shopfront_row(
        &mut scene,
        "right_city_shopfront_row_near",
        CityShopfrontRow {
            facing: FacadeFacing::Right,
            face_base: Vec3::new(11.8, 0.0, -34.8),
            length: 10.5,
            height: 7.8,
            bays: 3,
        },
        CityShopfrontMaterials {
            shell: oily_patched_concrete_material,
            panel: city_shopfront_material,
            awning: market_awning_material,
            trim: rain_black_trim_material,
            cool_light: 4,
            warm_light: 5,
        },
    );
    append_city_shopfront_row(
        &mut scene,
        "right_city_shopfront_row_far",
        CityShopfrontRow {
            facing: FacadeFacing::Right,
            face_base: Vec3::new(13.9, 0.0, -53.2),
            length: 12.5,
            height: 8.4,
            bays: 4,
        },
        CityShopfrontMaterials {
            shell: oily_patched_concrete_material,
            panel: city_shopfront_material,
            awning: market_awning_material,
            trim: rain_black_trim_material,
            cool_light: 4,
            warm_light: 5,
        },
    );
    append_city_shopfront_row(
        &mut scene,
        "cross_street_shopfront_back_left",
        CityShopfrontRow {
            facing: FacadeFacing::Back,
            face_base: Vec3::new(-7.2, 0.0, -47.8),
            length: 10.8,
            height: 7.4,
            bays: 3,
        },
        CityShopfrontMaterials {
            shell: oily_patched_concrete_material,
            panel: city_shopfront_material,
            awning: market_awning_material,
            trim: rain_black_trim_material,
            cool_light: 4,
            warm_light: 5,
        },
    );
    append_city_shopfront_row(
        &mut scene,
        "cross_street_shopfront_back_right",
        CityShopfrontRow {
            facing: FacadeFacing::Back,
            face_base: Vec3::new(7.2, 0.0, -49.8),
            length: 10.8,
            height: 7.4,
            bays: 3,
        },
        CityShopfrontMaterials {
            shell: oily_patched_concrete_material,
            panel: city_shopfront_material,
            awning: market_awning_material,
            trim: rain_black_trim_material,
            cool_light: 4,
            warm_light: 5,
        },
    );
    append_apartment_tower_cluster(
        &mut scene,
        "central_apartment_skyline",
        city_skyline_material,
        rain_black_trim_material,
        1,
        -67.5,
    );
    append_market_canopy(
        &mut scene,
        "left_market_overhead_tarp",
        Vec3::new(-5.2, 4.55, -36.8),
        Vec3::new(11.0, 0.26, 8.2),
        market_awning_material,
        rain_black_trim_material,
    );
    append_market_canopy(
        &mut scene,
        "center_bazaar_overhead_tarp",
        Vec3::new(0.6, 4.85, -48.4),
        Vec3::new(13.5, 0.28, 8.8),
        market_awning_material,
        rain_black_trim_material,
    );
    append_market_canopy(
        &mut scene,
        "right_market_overhead_tarp",
        Vec3::new(5.2, 4.55, -40.6),
        Vec3::new(11.0, 0.26, 8.2),
        market_awning_material,
        rain_black_trim_material,
    );

    scene.meshes.extend([
        detail_wall_mesh(
            "city_cross_street_cyan_sign",
            5.2,
            0.34,
            4,
            Vec2::new(1.0, 1.0),
            Mat4::translation(Vec3::new(-8.2, 5.85, -47.35)),
        ),
        detail_wall_mesh(
            "city_market_amber_sign",
            4.6,
            0.32,
            5,
            Vec2::new(1.0, 1.0),
            Mat4::translation(Vec3::new(8.7, 5.25, -49.35)),
        ),
        puddle_mesh(
            "city_cross_street_reflection_lake",
            5.8,
            7.8,
            puddle_material,
            Vec2::new(2.6, 3.2),
            Vec3::new(0.35, -0.047, -36.4),
        ),
        puddle_mesh(
            "left_market_lane_reflection_pool",
            3.6,
            7.0,
            puddle_material,
            Vec2::new(1.6, 3.0),
            Vec3::new(-9.6, -0.047, -45.2),
        ),
        puddle_mesh(
            "right_bazaar_lane_reflection_pool",
            3.6,
            7.0,
            puddle_material,
            Vec2::new(1.6, 3.0),
            Vec3::new(9.6, -0.047, -46.8),
        ),
    ]);

    for name in [
        "left_aged_brick",
        "right_aged_brick",
        "alley_end_brick",
        "left_loading_shutter",
        "right_loading_shutter",
        "left_recessed_service_bay",
        "far_scaffold_shadow",
        "left_city_shopfront_row_near_shell",
        "right_city_shopfront_row_near_shell",
        "cross_street_shopfront_back_left_shell",
        "cross_street_shopfront_back_right_shell",
        "central_apartment_skyline_tower_0_shell",
        "central_apartment_skyline_tower_2_shell",
        "central_apartment_skyline_tower_4_shell",
    ] {
        mark_prepass(&mut scene, name);
    }

    append_gltf_instances(
        &mut scene,
        root.join("models/Barrel_01/Barrel_01_1k.gltf"),
        "barrel",
        &[
            Mat4::translation(Vec3::new(-3.18, 0.02, -5.20)),
            Mat4::rotation_y(0.35) * Mat4::translation(Vec3::new(-2.58, 0.02, -5.68)),
            Mat4::rotation_y(-0.5) * Mat4::translation(Vec3::new(3.18, 0.02, -10.95)),
            Mat4::rotation_y(0.65) * Mat4::translation(Vec3::new(3.02, 0.02, -17.60)),
            Mat4::rotation_y(-0.20) * Mat4::translation(Vec3::new(-2.72, 0.02, -20.25)),
            Mat4::rotation_y(0.48) * Mat4::translation(Vec3::new(-8.20, 0.02, -35.35)),
            Mat4::rotation_y(-0.58) * Mat4::translation(Vec3::new(8.55, 0.02, -42.70)),
            Mat4::rotation_y(0.28) * Mat4::translation(Vec3::new(-10.80, 0.02, -52.80)),
        ],
    )?;
    append_gltf_instances(
        &mut scene,
        root.join("models/street_lamp_01/street_lamp_01_1k.gltf"),
        "street_lamp",
        &[
            Mat4::translation(Vec3::new(-3.48, 0.02, -0.75)),
            Mat4::translation(Vec3::new(3.68, 0.02, -6.65)),
            Mat4::translation(Vec3::new(-3.78, 0.02, -13.15)),
            Mat4::translation(Vec3::new(3.78, 0.02, -18.35)),
            Mat4::translation(Vec3::new(-3.86, 0.02, -23.20)),
            Mat4::translation(Vec3::new(-8.85, 0.02, -33.60)),
            Mat4::translation(Vec3::new(8.95, 0.02, -39.60)),
            Mat4::translation(Vec3::new(-10.65, 0.02, -51.80)),
            Mat4::translation(Vec3::new(10.80, 0.02, -58.20)),
        ],
    )?;
    append_gltf_instances(
        &mut scene,
        root.join("models/concrete_road_barrier/concrete_road_barrier_1k.gltf"),
        "barrier",
        &[
            Mat4::rotation_y(-0.22) * Mat4::translation(Vec3::new(0.85, 0.0, -6.2)),
            Mat4::rotation_y(0.18) * Mat4::translation(Vec3::new(0.65, 0.0, -11.3)),
            Mat4::rotation_y(-0.38) * Mat4::translation(Vec3::new(-0.65, 0.0, -15.85)),
            Mat4::rotation_y(0.26) * Mat4::translation(Vec3::new(0.35, 0.0, -21.20)),
            Mat4::rotation_y(0.92) * Mat4::translation(Vec3::new(-4.20, 0.0, -34.60)),
            Mat4::rotation_y(-0.82) * Mat4::translation(Vec3::new(4.80, 0.0, -39.40)),
            Mat4::rotation_y(0.14) * Mat4::translation(Vec3::new(1.35, 0.0, -52.80)),
        ],
    )?;
    append_gltf_instances(
        &mut scene,
        root.join("models/steel_frame_shelves_01/steel_frame_shelves_01_1k.gltf"),
        "shelving",
        &[
            Mat4::scale(Vec3::splat(0.1))
                * Mat4::rotation_y(90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(-3.82, 0.02, -3.95)),
            Mat4::scale(Vec3::splat(0.1))
                * Mat4::rotation_y(90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(-3.84, 0.02, -6.65)),
            Mat4::scale(Vec3::splat(0.1))
                * Mat4::rotation_y(-90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(3.82, 0.02, -18.80)),
            Mat4::scale(Vec3::splat(0.1))
                * Mat4::rotation_y(90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(-8.95, 0.02, -38.20)),
            Mat4::scale(Vec3::splat(0.1))
                * Mat4::rotation_y(-90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(8.95, 0.02, -47.60)),
        ],
    )?;
    append_gltf_instances(
        &mut scene,
        root.join("models/utility_box_01/utility_box_01_1k.gltf"),
        "utility_box",
        &[
            Mat4::rotation_y(-90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(4.12, 0.05, -2.35)),
            Mat4::rotation_y(90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(-4.12, 0.05, -13.85)),
            Mat4::rotation_y(-90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(4.10, 0.05, -20.95)),
            Mat4::rotation_y(90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(-4.10, 0.05, -22.60)),
            Mat4::rotation_y(90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(-9.40, 0.05, -35.20)),
            Mat4::rotation_y(-90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(9.40, 0.05, -44.60)),
            Mat4::rotation_y(90.0_f32.to_radians())
                * Mat4::translation(Vec3::new(-11.80, 0.05, -56.20)),
        ],
    )?;
    append_gltf_instances(
        &mut scene,
        root.join("models/old_tyre/old_tyre_1k.gltf"),
        "old_tyre",
        &[
            Mat4::rotation_y(0.22) * Mat4::translation(Vec3::new(-3.35, 0.34, -8.85)),
            Mat4::rotation_y(-0.35) * Mat4::translation(Vec3::new(3.32, 0.34, -12.85)),
            Mat4::rotation_y(0.90) * Mat4::translation(Vec3::new(2.92, 0.34, -18.95)),
            Mat4::rotation_y(-0.72) * Mat4::translation(Vec3::new(-3.05, 0.34, -23.45)),
            Mat4::rotation_y(0.55) * Mat4::translation(Vec3::new(-7.85, 0.34, -37.75)),
            Mat4::rotation_y(-0.88) * Mat4::translation(Vec3::new(8.15, 0.34, -50.65)),
        ],
    )?;
    append_gltf_instances(
        &mut scene,
        root.join("models/metal_trash_can/metal_trash_can_1k.gltf"),
        "trash_can",
        &[
            Mat4::rotation_y(-0.3) * Mat4::translation(Vec3::new(3.28, 0.02, -4.15)),
            Mat4::rotation_y(0.5) * Mat4::translation(Vec3::new(-3.28, 0.02, -14.05)),
            Mat4::rotation_y(-0.68) * Mat4::translation(Vec3::new(3.10, 0.02, -16.90)),
            Mat4::rotation_y(0.42) * Mat4::translation(Vec3::new(-3.18, 0.02, -21.80)),
            Mat4::rotation_y(-0.35) * Mat4::translation(Vec3::new(8.35, 0.02, -35.70)),
            Mat4::rotation_y(0.62) * Mat4::translation(Vec3::new(-8.10, 0.02, -46.25)),
            Mat4::rotation_y(-0.48) * Mat4::translation(Vec3::new(10.25, 0.02, -54.40)),
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

#[derive(Clone, Copy)]
struct TexturedGeneratedMaterialParams {
    class: MaterialClass,
    base_color: Color,
    roughness: f32,
    metallic: f32,
    specular_intensity: f32,
    cavity: f32,
    material_id: u8,
    emission: Color,
}

#[derive(Clone, Copy)]
struct GeneratedTextureSetMaterialParams {
    class: MaterialClass,
    base_color: Color,
    roughness: f32,
    metallic: f32,
    specular_intensity: f32,
    cavity: f32,
    material_id: u8,
}

fn load_generated_texture(root: &Path, filename: &str) -> Result<TextureImage, GltfImportError> {
    let path = root.join("generated").join(filename);
    TextureImage::from_path(&path)
        .map_err(|message| GltfImportError::Gltf(format!("{}: {message}", path.display())))
}

fn load_generated_material_texture(
    root: &Path,
    filename: &str,
) -> Result<TextureImage, GltfImportError> {
    let path = root.join("generated").join("textures").join(filename);
    TextureImage::from_path(&path)
        .map_err(|message| GltfImportError::Gltf(format!("{}: {message}", path.display())))
}

fn textured_generated_material(
    name: &str,
    texture: TextureImage,
    params: TexturedGeneratedMaterialParams,
) -> Material {
    let emissive = params.emission != Color::BLACK;
    Material {
        name: name.to_string(),
        class: params.class,
        base_color: params.base_color,
        base_color_texture: Some(texture),
        normal_texture: None,
        pbr_texture: None,
        roughness: params.roughness,
        metallic: params.metallic,
        specular_intensity: params.specular_intensity,
        cavity: params.cavity,
        material_id: params.material_id,
        texture_sample_count: 1,
        flags: MaterialFlags {
            emissive,
            receives_contact_shadows: !emissive,
            uses_bent_normals: params.specular_intensity > 0.15,
            ..MaterialFlags::default()
        },
        emission: params.emission,
    }
}

fn generated_texture_set_material(
    root: &Path,
    name: &str,
    stem: &str,
    params: GeneratedTextureSetMaterialParams,
) -> Result<Material, GltfImportError> {
    Ok(Material {
        name: name.to_string(),
        class: params.class,
        base_color: params.base_color,
        base_color_texture: Some(load_generated_material_texture(
            root,
            &format!("{stem}_base.png"),
        )?),
        normal_texture: Some(load_generated_material_texture(
            root,
            &format!("{stem}_normal.png"),
        )?),
        pbr_texture: Some(load_generated_material_texture(
            root,
            &format!("{stem}_arm.png"),
        )?),
        roughness: params.roughness,
        metallic: params.metallic,
        specular_intensity: params.specular_intensity,
        cavity: params.cavity,
        material_id: params.material_id,
        texture_sample_count: 3,
        flags: MaterialFlags {
            receives_contact_shadows: true,
            uses_bent_normals: true,
            ..MaterialFlags::default()
        },
        emission: Color::BLACK,
    })
}

#[derive(Clone, Copy)]
struct SolidMaterialParams {
    class: MaterialClass,
    base_color: Color,
    roughness: f32,
    metallic: f32,
    specular_intensity: f32,
    cavity: f32,
    material_id: u8,
}

fn solid_material(name: &str, params: SolidMaterialParams) -> Material {
    Material {
        name: name.to_string(),
        class: params.class,
        base_color: params.base_color,
        base_color_texture: None,
        normal_texture: None,
        pbr_texture: None,
        roughness: params.roughness,
        metallic: params.metallic,
        specular_intensity: params.specular_intensity,
        cavity: params.cavity,
        material_id: params.material_id,
        texture_sample_count: 1,
        flags: MaterialFlags {
            glass: params.class == MaterialClass::Glass,
            receives_contact_shadows: true,
            ..MaterialFlags::default()
        },
        emission: Color::BLACK,
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
        prepass_eligible: false,
        reflection_proxy: true,
    }
}

fn puddle_mesh(
    name: &str,
    width: f32,
    depth: f32,
    material_index: usize,
    uv_scale: Vec2,
    position: Vec3,
) -> Mesh {
    let outline = [
        Vec2::new(-0.56, -0.10),
        Vec2::new(-0.49, -0.30),
        Vec2::new(-0.35, -0.44),
        Vec2::new(-0.12, -0.53),
        Vec2::new(0.10, -0.50),
        Vec2::new(0.31, -0.42),
        Vec2::new(0.49, -0.24),
        Vec2::new(0.55, -0.03),
        Vec2::new(0.47, 0.15),
        Vec2::new(0.32, 0.31),
        Vec2::new(0.09, 0.45),
        Vec2::new(-0.14, 0.51),
        Vec2::new(-0.36, 0.42),
        Vec2::new(-0.52, 0.24),
        Vec2::new(-0.60, 0.06),
    ];
    let mut vertices = Vec::with_capacity(outline.len() + 1);
    vertices.push(Vertex {
        position: Vec3::ZERO,
        normal: Vec3::Y,
        tangent: Vec3::X,
        tangent_sign: 1.0,
        uv: uv_scale * 0.5,
        color: Color::WHITE,
    });
    for point in outline {
        vertices.push(Vertex {
            position: Vec3::new(point.x * width, 0.0, point.y * depth),
            normal: Vec3::Y,
            tangent: Vec3::X,
            tangent_sign: 1.0,
            uv: Vec2::new((point.x + 0.5) * uv_scale.x, (point.y + 0.5) * uv_scale.y),
            color: Color::WHITE,
        });
    }
    let mut indices = Vec::with_capacity(outline.len());
    for index in 1..=outline.len() as u32 {
        let next = if index == outline.len() as u32 {
            1
        } else {
            index + 1
        };
        indices.push([0, next, index]);
    }
    Mesh {
        name: name.to_string(),
        vertices,
        indices,
        material_index,
        transform: Mat4::translation(position),
        shadow_class: ShadowCasterClass::None,
        is_floor_or_ground: true,
        prepass_eligible: false,
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
        prepass_eligible: false,
        reflection_proxy: true,
    }
}

fn detail_wall_mesh(
    name: &str,
    width: f32,
    height: f32,
    material_index: usize,
    uv_scale: Vec2,
    transform: Mat4,
) -> Mesh {
    let mut mesh = wall_mesh(name, width, height, material_index, uv_scale, transform);
    mesh.prepass_eligible = false;
    mesh.reflection_proxy = false;
    mesh
}

fn mark_prepass(scene: &mut RenderScene, name: &str) {
    if let Some(mesh) = scene.meshes.iter_mut().find(|mesh| mesh.name == name) {
        mesh.prepass_eligible = true;
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

#[derive(Clone, Copy)]
enum FacadeFacing {
    Left,
    Right,
    Back,
}

impl FacadeFacing {
    fn normal(self) -> Vec3 {
        match self {
            Self::Left => Vec3::X,
            Self::Right => -Vec3::X,
            Self::Back => Vec3::Z,
        }
    }

    fn tangent(self) -> Vec3 {
        match self {
            Self::Left | Self::Right => Vec3::Z,
            Self::Back => Vec3::X,
        }
    }

    fn box_size(self, facade_width: f32, height: f32, depth: f32) -> Vec3 {
        match self {
            Self::Left | Self::Right => Vec3::new(depth, height, facade_width),
            Self::Back => Vec3::new(facade_width, height, depth),
        }
    }
}

#[derive(Clone, Copy)]
struct CityShopfrontMaterials {
    shell: usize,
    panel: usize,
    awning: usize,
    trim: usize,
    cool_light: usize,
    warm_light: usize,
}

#[derive(Clone, Copy)]
struct CityShopfrontRow {
    facing: FacadeFacing,
    face_base: Vec3,
    length: f32,
    height: f32,
    bays: usize,
}

fn append_city_shopfront_row(
    scene: &mut RenderScene,
    prefix: &str,
    row: CityShopfrontRow,
    materials: CityShopfrontMaterials,
) {
    let CityShopfrontRow {
        facing,
        face_base,
        length,
        height,
        bays,
    } = row;
    let normal = facing.normal();
    let tangent = facing.tangent();
    let shell_depth = 0.72;
    let shell_center = face_base + Vec3::Y * (height * 0.5) - normal * (shell_depth * 0.5);
    scene.meshes.push(facade_box_mesh(
        &format!("{prefix}_shell"),
        FacadeBoxSpec {
            facing,
            facade_width: length,
            height,
            depth: shell_depth,
            material_index: materials.shell,
            uv_scale: Vec2::new(length * 0.55, height * 0.35),
            center: shell_center,
            prepass_eligible: true,
            reflection_proxy: true,
        },
    ));

    let bay_spacing = length / bays as f32;
    let bay_width = bay_spacing * 0.68;
    for bay in 0..bays {
        let u = -length * 0.5 + bay_spacing * (bay as f32 + 0.5);
        let light_material = if bay % 2 == 0 {
            materials.cool_light
        } else {
            materials.warm_light
        };

        scene.meshes.push(facade_box_mesh(
            &format!("{prefix}_recess_{}", bay),
            FacadeBoxSpec {
                facing,
                facade_width: bay_width * 1.08,
                height: 3.05,
                depth: 0.22,
                material_index: materials.trim,
                uv_scale: Vec2::new(1.0, 1.0),
                center: face_base + tangent * u + Vec3::Y * 1.72 + normal * 0.08,
                prepass_eligible: false,
                reflection_proxy: false,
            },
        ));
        scene.meshes.push(facade_box_mesh(
            &format!("{prefix}_shutter_{}", bay),
            FacadeBoxSpec {
                facing,
                facade_width: bay_width,
                height: 2.42,
                depth: 0.28,
                material_index: materials.panel,
                uv_scale: Vec2::new(1.0, 1.0),
                center: face_base + tangent * u + Vec3::Y * 1.60 + normal * 0.24,
                prepass_eligible: false,
                reflection_proxy: false,
            },
        ));
        scene.meshes.push(facade_box_mesh(
            &format!("{prefix}_upper_window_{}", bay),
            FacadeBoxSpec {
                facing,
                facade_width: bay_width * 0.72,
                height: 1.22,
                depth: 0.18,
                material_index: materials.panel,
                uv_scale: Vec2::new(1.0, 1.0),
                center: face_base + tangent * u + Vec3::Y * 5.20 + normal * 0.18,
                prepass_eligible: false,
                reflection_proxy: false,
            },
        ));
        scene.meshes.push(facade_box_mesh(
            &format!("{prefix}_projecting_awning_{}", bay),
            FacadeBoxSpec {
                facing,
                facade_width: bay_width * 1.08,
                height: 0.22,
                depth: 1.25,
                material_index: materials.awning,
                uv_scale: Vec2::new(1.0, 1.0),
                center: face_base + tangent * u + Vec3::Y * 3.22 + normal * 0.62,
                prepass_eligible: false,
                reflection_proxy: false,
            },
        ));
        scene.meshes.push(facade_box_mesh(
            &format!("{prefix}_neon_tube_{}", bay),
            FacadeBoxSpec {
                facing,
                facade_width: bay_width * 0.74,
                height: 0.10,
                depth: 0.16,
                material_index: light_material,
                uv_scale: Vec2::new(1.0, 1.0),
                center: face_base + tangent * u + Vec3::Y * 2.93 + normal * 0.73,
                prepass_eligible: false,
                reflection_proxy: false,
            },
        ));
    }

    for post in 0..=bays {
        let u = -length * 0.5 + bay_spacing * post as f32;
        scene.meshes.push(facade_box_mesh(
            &format!("{prefix}_structural_post_{}", post),
            FacadeBoxSpec {
                facing,
                facade_width: 0.14,
                height: height * 0.92,
                depth: 0.22,
                material_index: materials.trim,
                uv_scale: Vec2::new(1.0, 1.0),
                center: face_base + tangent * u + Vec3::Y * (height * 0.46) + normal * 0.14,
                prepass_eligible: false,
                reflection_proxy: false,
            },
        ));
    }
}

fn append_market_canopy(
    scene: &mut RenderScene,
    prefix: &str,
    center: Vec3,
    size: Vec3,
    awning_material: usize,
    trim_material: usize,
) {
    scene.meshes.push(box_mesh(
        &format!("{prefix}_slab"),
        size,
        awning_material,
        Vec2::new(1.0, 1.0),
        Mat4::translation(center),
        false,
        false,
    ));
    let pole_y = center.y - 1.95;
    for (index, (x, z)) in [
        (-size.x * 0.44, -size.z * 0.42),
        (size.x * 0.44, -size.z * 0.42),
        (-size.x * 0.44, size.z * 0.42),
        (size.x * 0.44, size.z * 0.42),
    ]
    .into_iter()
    .enumerate()
    {
        scene.meshes.push(box_mesh(
            &format!("{prefix}_pole_{}", index),
            Vec3::new(0.13, 3.9, 0.13),
            trim_material,
            Vec2::new(1.0, 1.0),
            Mat4::translation(Vec3::new(center.x + x, pole_y, center.z + z)),
            false,
            false,
        ));
    }
    scene.meshes.push(box_mesh(
        &format!("{prefix}_front_gutter"),
        Vec3::new(size.x * 0.96, 0.18, 0.18),
        trim_material,
        Vec2::new(1.0, 1.0),
        Mat4::translation(Vec3::new(
            center.x,
            center.y - 0.10,
            center.z + size.z * 0.52,
        )),
        false,
        false,
    ));
}

fn append_apartment_tower_cluster(
    scene: &mut RenderScene,
    prefix: &str,
    skyline_material: usize,
    trim_material: usize,
    shell_material: usize,
    base_z: f32,
) {
    let towers = [
        (-11.5, 4.2, 12.0, 3.0, 0.0),
        (-6.8, 5.2, 16.0, 3.6, -1.2),
        (-1.2, 4.6, 13.5, 3.2, 0.8),
        (4.2, 5.8, 17.5, 3.8, -0.6),
        (10.4, 4.8, 14.5, 3.3, 0.6),
    ];
    for (index, (x, width, height, depth, z_offset)) in towers.into_iter().enumerate() {
        let center = Vec3::new(x, height * 0.5, base_z + z_offset);
        scene.meshes.push(box_mesh(
            &format!("{prefix}_tower_{}_shell", index),
            Vec3::new(width, height, depth),
            shell_material,
            Vec2::new(width * 0.4, height * 0.25),
            Mat4::translation(center),
            index % 2 == 0,
            true,
        ));
        scene.meshes.push(facade_box_mesh(
            &format!("{prefix}_tower_{}_window_grid", index),
            FacadeBoxSpec {
                facing: FacadeFacing::Back,
                facade_width: width * 0.70,
                height: height * 0.56,
                depth: 0.14,
                material_index: skyline_material,
                uv_scale: Vec2::new(1.0, 1.0),
                center: Vec3::new(x, height * 0.56, base_z + z_offset + depth * 0.5 + 0.08),
                prepass_eligible: false,
                reflection_proxy: false,
            },
        ));
        scene.meshes.push(box_mesh(
            &format!("{prefix}_tower_{}_roof_tank", index),
            Vec3::new(width * 0.28, 0.55, depth * 0.26),
            trim_material,
            Vec2::new(1.0, 1.0),
            Mat4::translation(Vec3::new(
                x - width * 0.22,
                height + 0.35,
                base_z + z_offset,
            )),
            false,
            false,
        ));
    }
}

#[derive(Clone, Copy)]
struct FacadeBoxSpec {
    facing: FacadeFacing,
    facade_width: f32,
    height: f32,
    depth: f32,
    material_index: usize,
    uv_scale: Vec2,
    center: Vec3,
    prepass_eligible: bool,
    reflection_proxy: bool,
}

fn facade_box_mesh(name: &str, spec: FacadeBoxSpec) -> Mesh {
    box_mesh(
        name,
        spec.facing
            .box_size(spec.facade_width, spec.height, spec.depth),
        spec.material_index,
        spec.uv_scale,
        Mat4::translation(spec.center),
        spec.prepass_eligible,
        spec.reflection_proxy,
    )
}

fn box_mesh(
    name: &str,
    size: Vec3,
    material_index: usize,
    uv_scale: Vec2,
    transform: Mat4,
    prepass_eligible: bool,
    reflection_proxy: bool,
) -> Mesh {
    let hx = size.x * 0.5;
    let hy = size.y * 0.5;
    let hz = size.z * 0.5;
    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(12);
    let mut push_face = |corners: [Vec3; 4], normal: Vec3, tangent: Vec3| {
        let base = vertices.len() as u32;
        let uvs = [
            Vec2::new(0.0, 0.0),
            Vec2::new(uv_scale.x, 0.0),
            uv_scale,
            Vec2::new(0.0, uv_scale.y),
        ];
        for (position, uv) in corners.into_iter().zip(uvs) {
            vertices.push(Vertex {
                position,
                normal,
                tangent,
                tangent_sign: 1.0,
                uv,
                color: Color::WHITE,
            });
        }
        indices.push([base, base + 1, base + 2]);
        indices.push([base, base + 2, base + 3]);
    };

    push_face(
        [
            Vec3::new(-hx, -hy, hz),
            Vec3::new(hx, -hy, hz),
            Vec3::new(hx, hy, hz),
            Vec3::new(-hx, hy, hz),
        ],
        Vec3::Z,
        Vec3::X,
    );
    push_face(
        [
            Vec3::new(hx, -hy, -hz),
            Vec3::new(-hx, -hy, -hz),
            Vec3::new(-hx, hy, -hz),
            Vec3::new(hx, hy, -hz),
        ],
        -Vec3::Z,
        -Vec3::X,
    );
    push_face(
        [
            Vec3::new(hx, -hy, hz),
            Vec3::new(hx, -hy, -hz),
            Vec3::new(hx, hy, -hz),
            Vec3::new(hx, hy, hz),
        ],
        Vec3::X,
        -Vec3::Z,
    );
    push_face(
        [
            Vec3::new(-hx, -hy, -hz),
            Vec3::new(-hx, -hy, hz),
            Vec3::new(-hx, hy, hz),
            Vec3::new(-hx, hy, -hz),
        ],
        -Vec3::X,
        Vec3::Z,
    );
    push_face(
        [
            Vec3::new(-hx, hy, hz),
            Vec3::new(hx, hy, hz),
            Vec3::new(hx, hy, -hz),
            Vec3::new(-hx, hy, -hz),
        ],
        Vec3::Y,
        Vec3::X,
    );
    push_face(
        [
            Vec3::new(-hx, -hy, -hz),
            Vec3::new(hx, -hy, -hz),
            Vec3::new(hx, -hy, hz),
            Vec3::new(-hx, -hy, hz),
        ],
        -Vec3::Y,
        Vec3::X,
    );

    Mesh {
        name: name.to_string(),
        vertices,
        indices,
        material_index,
        transform,
        shadow_class: ShadowCasterClass::Main,
        is_floor_or_ground: false,
        prepass_eligible,
        reflection_proxy,
    }
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
