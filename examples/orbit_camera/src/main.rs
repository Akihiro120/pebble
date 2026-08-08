use pebble::prelude::*;
use pebble::wgpu::prelude::*;
use pebble::wgpu::{
    backend::WGPUPlugin,
    instance::{GPUMaterialInstance, MaterialInstance, MaterialInstanceBuilder},
    material::{GPUMaterial, Material, MaterialBuilder},
    mesh::{GPUMesh, Mesh, MeshBuilder, Vertex},
    textures::{Texture, TextureBuilder},
    window::WinitWindow,
};

const SHADER: &str = r#"
struct Camera {
    proj: mat4x4<f32>,
    view: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> camera: Camera;

struct VOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) normal: vec3<f32>,
};

@vertex
fn vs_main(@location(0) pos: vec3<f32>, @location(1) uv: vec2<f32>, @location(2) normal: vec3<f32>) -> VOut {
    var out: VOut;
    out.world_pos = pos;
    out.uv = uv;
    out.normal = normal;
    out.clip_pos = camera.proj * camera.view * vec4<f32>(pos, 1.0);
    return out;
}

@group(1) @binding(0) var albedo: texture_2d<f32>;
@group(1) @binding(1) var albedo_sampler: sampler;

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    let color = textureSample(albedo, albedo_sampler, in.uv).rgb;
    let light_pos = vec3<f32>(1.0, 3.0, -2.0);
    let light_color = vec3<f32>(1.0, 1.0, 1.0);

    let ambient_strength = 0.2;
    let ambient = ambient_strength * light_color;

    let norm = normalize(in.normal);
    let light_dir = normalize(light_pos - in.world_pos);
    let diff = max(dot(norm, light_dir), 0.0);
    let diffuse = diff * light_color;

    let result = (ambient + diffuse) * color;
    return vec4<f32>(result, 1.0);
}
"#;

#[repr(C)]
#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    proj: glam::Mat4,
    view: glam::Mat4,
}

/// A camera's own bind group (view/projection), built once the device
/// exists — see the book's "Custom GPU Resources" page for the general
/// pattern this follows.
struct Camera {
    buffer: Buffer,
    bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
}

impl LazyResource<WGPUBackend> for Camera {
    type Deps<'a> = ();

    fn construct<'a>(backend: &WGPUBackend, _deps: &()) -> Option<Self> {
        let bind_group_layout = BindGroupLayoutBuilder::new()
            .label("camera_layout")
            .entry("camera", 0, BindingKind::uniform_buffer(ShaderStages::VERTEX))
            .build(backend);

        let buffer = BufferBuilder::empty(std::mem::size_of::<CameraUniform>() as u64)
            .label("camera")
            .uniform()
            .build(backend);

        let bind_group = BindGroupBuilder::new(&bind_group_layout)
            .label("camera_bind_group")
            .buffer(&buffer)
            .build(backend);

        Some(Camera { buffer, bind_group_layout, bind_group })
    }
}

struct DepthTexture {
    view: TextureView,
}

impl LazyResource<WGPUBackend> for DepthTexture {
    type Deps<'a> = ();

    fn construct<'a>(backend: &WGPUBackend, _deps: &()) -> Option<Self> {
        let view = RenderTargetTextureBuilder::new(backend.surface_width(), backend.surface_height(), TextureFormat::Depth16Unorm)
            .label("depth")
            .usage(TextureUsages::RENDER_ATTACHMENT)
            .build(backend);
        Some(DepthTexture { view })
    }
}

#[derive(Copy, Clone, Default)]
struct TransformComponent {
    position: glam::Vec3,
    rotation: glam::Vec3,
    scale: glam::Vec3,
}

struct CameraComponent {
    uniform: CameraUniform,
}

/// Accumulated orbit angle, advanced by [`advance_orbit`] instead of read
/// straight off a wall clock — that's what lets [`toggle_pause`] freeze it
/// with a plain `bool` rather than having to fudge a "paused at" timestamp.
struct OrbitState {
    angle: f32,
    paused: bool,
}

struct CameraPlugin;
impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_resource(OrbitState { angle: 0.0, paused: false }).add_systems(
            SystemStage::Update,
            (toggle_pause, toggle_cursor, advance_orbit, update_camera, update_camera_buffer),
        );
    }
}

/// `Res<Input>` — a plain resource `WGPUPlugin` inserts automatically, no
/// backend type to name. `key_pressed` is edge-triggered (true only the
/// step Space goes down), unlike `key_held`, so this toggles once per press
/// rather than flickering every frame the key remains down.
fn toggle_pause(input: Res<Input>, mut state: ResMut<OrbitState>) {
    if input.key_pressed(KeyCode::Space) {
        state.paused = !state.paused;
    }
}

/// `Res<Window>` — Pebble's own opaque wrapper around the OS window
/// (`pebble::wgpu::prelude::Window`, not `winit::window::Window`).
/// `WGPUPlugin` inserts it the same way it inserts `Input`. `C` hides the
/// cursor and locks it in place, the way a captured-mouse camera would;
/// `Local<bool>` tracks which state we're in without needing a shared
/// resource just for that one flag (see the book's Resources page).
fn toggle_cursor(input: Res<Input>, window: Res<Window>, mut hidden: Local<bool>) {
    if input.key_pressed(KeyCode::KeyC) {
        *hidden = !*hidden;
        window.set_cursor_visible(!*hidden);
        window.set_cursor_grab(if *hidden { CursorGrabMode::Locked } else { CursorGrabMode::None });
    }
}

fn advance_orbit(time: Res<Time>, mut state: ResMut<OrbitState>) {
    if !state.paused {
        state.angle += time.delta_seconds();
    }
}

fn update_camera(
    window: Res<WindowResource<WinitWindow>>,
    orbit: Res<OrbitState>,
    mut query: Query<(&mut CameraComponent, &TransformComponent)>,
) {
    let size = window.handle.inner_size();
    let aspect_ratio = size.width as f32 / size.height as f32;

    for (active_camera, _transform) in query.iter() {
        active_camera.uniform.proj = glam::Mat4::perspective_rh(45.0f32.to_radians(), aspect_ratio, 0.1, 100.0);

        // Circle in the X-Z plane at a fixed height, so the camera orbits
        // level around the origin instead of also bobbing up and down.
        let radius = 5.0;
        let cam_x = f32::sin(orbit.angle) * radius;
        let cam_y = 2.0;
        let cam_z = f32::cos(orbit.angle) * radius;
        active_camera.uniform.view = glam::Mat4::look_at_rh(
            glam::Vec3::new(cam_x, cam_y, cam_z),
            glam::Vec3::default(),
            glam::Vec3::Y,
        );
    }
}

fn update_camera_buffer(camera: Option<Res<Camera>>, mut query: Query<&CameraComponent>) {
    let Some(camera) = camera else { return };
    for active_camera in query.iter() {
        camera.buffer.write(bytemuck::bytes_of(&active_camera.uniform));
    }
}

fn material_entries() -> Vec<BindingEntry> {
    vec![
        BindingEntry { name: "albedo", binding: 0, kind: BindingKind::texture_2d(ShaderStages::FRAGMENT) },
        BindingEntry { name: "albedo_sampler", binding: 1, kind: BindingKind::sampler(ShaderStages::FRAGMENT) },
    ]
}

fn cube_vertices() -> Vec<Vertex> {
    // Unused by this shader (only locations 0/1/2 are read) — a fixed
    // placeholder, same convention as the other examples.
    let tangent = glam::Vec4::new(1.0, 0.0, 0.0, 1.0);
    let v = |x: f32, y: f32, z: f32, u: f32, t: f32, nx: f32, ny: f32, nz: f32| {
        Vertex::new(glam::Vec3::new(x, y, z), glam::Vec2::new(u, t), glam::Vec3::new(nx, ny, nz), tangent)
    };
    vec![
        // +Z
        v(-0.5, 0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 1.0),
        v(-0.5, -0.5, 0.5, 0.0, 1.0, 0.0, 0.0, 1.0),
        v(0.5, -0.5, 0.5, 1.0, 1.0, 0.0, 0.0, 1.0),
        v(0.5, 0.5, 0.5, 1.0, 0.0, 0.0, 0.0, 1.0),
        // -Z
        v(0.5, 0.5, -0.5, 0.0, 0.0, 0.0, 0.0, -1.0),
        v(0.5, -0.5, -0.5, 0.0, 1.0, 0.0, 0.0, -1.0),
        v(-0.5, -0.5, -0.5, 1.0, 1.0, 0.0, 0.0, -1.0),
        v(-0.5, 0.5, -0.5, 1.0, 0.0, 0.0, 0.0, -1.0),
        // +Y
        v(-0.5, 0.5, -0.5, 0.0, 0.0, 0.0, 1.0, 0.0),
        v(-0.5, 0.5, 0.5, 0.0, 1.0, 0.0, 1.0, 0.0),
        v(0.5, 0.5, 0.5, 1.0, 1.0, 0.0, 1.0, 0.0),
        v(0.5, 0.5, -0.5, 1.0, 0.0, 0.0, 1.0, 0.0),
        // -Y
        v(-0.5, -0.5, 0.5, 0.0, 0.0, 0.0, -1.0, 0.0),
        v(-0.5, -0.5, -0.5, 0.0, 1.0, 0.0, -1.0, 0.0),
        v(0.5, -0.5, -0.5, 1.0, 1.0, 0.0, -1.0, 0.0),
        v(0.5, -0.5, 0.5, 1.0, 0.0, 0.0, -1.0, 0.0),
        // +X
        v(0.5, 0.5, 0.5, 0.0, 0.0, 1.0, 0.0, 0.0),
        v(0.5, -0.5, 0.5, 0.0, 1.0, 1.0, 0.0, 0.0),
        v(0.5, -0.5, -0.5, 1.0, 1.0, 1.0, 0.0, 0.0),
        v(0.5, 0.5, -0.5, 1.0, 0.0, 1.0, 0.0, 0.0),
        // -X
        v(-0.5, 0.5, -0.5, 0.0, 0.0, -1.0, 0.0, 0.0),
        v(-0.5, -0.5, -0.5, 0.0, 1.0, -1.0, 0.0, 0.0),
        v(-0.5, -0.5, 0.5, 1.0, 1.0, -1.0, 0.0, 0.0),
        v(-0.5, 0.5, 0.5, 1.0, 0.0, -1.0, 0.0, 0.0),
    ]
}

const INDICES: [u32; 36] = [
    0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7, 8, 9, 10, 8, 10, 11, 12, 13, 14, 12, 14, 15, 16, 17, 18, 16, 18, 19, 20, 21,
    22, 20, 22, 23,
];

fn main() {
    tracing_subscriber::fmt::init();
    App::new()
        .add_plugin(WGPUPlugin::new(WindowConfig {
            title: "Orbiting Camera".to_string(),
            width: 1920,
            height: 1080,
        }))
        .add_plugin(LazyResourcePlugin::<WGPUBackend, DepthTexture>::new())
        .add_plugin(LazyResourcePlugin::<WGPUBackend, Camera>::new())
        .add_plugin(CameraPlugin)
        .add_system(SystemStage::PreUpdate, register_camera_layout.once())
        .add_system(SystemStage::PreUpdate, setup.once())
        .add_system(SystemStage::Render, render)
        .build()
        .run();
}

/// Registers the camera's bind group layout into the global pool as soon as `Camera` exists —
/// decoupled from `setup`, which never needs `Res<Camera>` at all: `GroupEntry::Global`
/// resolves "camera" lazily at upload time, so it doesn't matter which of these two `.once()`
/// systems happens to run first.
fn register_camera_layout(camera: Res<Camera>, mut pool: ResMut<GlobalLayoutPool>) -> Option<()> {
    pool.register("camera", camera.bind_group_layout.clone());
    Some(())
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<Material>>,
    mut textures: ResMut<Assets<Texture>>,
    mut instances: ResMut<Assets<MaterialInstance>>,
    backend: Res<WGPUBackend>,
) -> Option<()> {
    let cube = MeshBuilder::new(cube_vertices(), INDICES.to_vec()).build_asset("cube", &mut meshes);

    let brick = TextureBuilder::from_file("../assets/textures/brick.png").build_asset("brick", &mut textures);

    let material = MaterialBuilder::new(SHADER)
        .label("lit")
        .vertex_layouts(vec![Vertex::layout()])
        .entries(vec![
            GroupEntry::Global("camera"),         // @group(0), resolved from the global pool
            GroupEntry::Own(material_entries()),  // @group(1): texture/sampler
        ])
        .targets(vec![ColorTargetState {
            format: backend.surface_format(),
            blend: None,
            write_mask: Default::default(),
        }])
        .depth(DepthStencilState {
            format: TextureFormat::Depth16Unorm,
            depth_write_enabled: Some(true),
            depth_compare: Some(CompareFunction::Less),
            stencil: StencilState::default(),
            bias: DepthBiasState::default(),
        })
        .build_asset("lit", &mut materials);

    let cube_instance = MaterialInstanceBuilder::new(material)
        .texture("albedo", brick)
        .sampler("albedo_sampler", SamplerKind::LinearRepeat)
        .build_asset("cube_brick", &mut instances);

    commands.spawn((
        CameraComponent { uniform: CameraUniform::default() },
        TransformComponent { position: glam::Vec3::new(0.0, 0.0, -3.0), ..Default::default() },
    ));
    commands.spawn((cube, cube_instance));
    Some(())
}

fn render(
    mut frame: ResMut<CurrentFrame<WGPUBackend>>,
    materials: Res<ProcessedAssets<GPUMaterial>>,
    meshes: Res<ProcessedAssets<GPUMesh>>,
    instances: Res<ProcessedAssets<GPUMaterialInstance>>,
    camera: Option<Res<Camera>>,
    depth: Option<Res<DepthTexture>>,
    mut query: Query<(&Handle<Mesh>, &Handle<MaterialInstance>)>,
) {
    let Some(camera) = camera else { return };
    let Some(depth) = depth else { return };
    let Some(mut active) = frame.active() else { return };

    let mut pass = active.begin_pass(Pass {
        colors: &[ColorTarget::default([0.2, 0.3, 0.3, 1.0])],
        depth: Some(DepthTarget::new(&depth.view, 1.0)),
    });
    pass.set_bind_group(0, &camera.bind_group, &[]); // group 0: shared across every draw

    for (mesh_handle, instance_handle) in query.iter() {
        let Some(mesh) = meshes.get(mesh_handle.id) else { continue };
        let Some(instance) = instances.get(instance_handle.id) else { continue };
        let Some(material) = materials.get(instance.target) else { continue };

        pass.set_pipeline(&material.pipeline);
        pass.set_bind_group(1, &instance.bind_group, &[]); // group 1: per-instance
        pass.set_vertex_buffer(0, &mesh.vertex_buffer);
        pass.set_index_buffer(&mesh.index_buffer, IndexFormat::Uint32);
        pass.draw_indexed(0..mesh.index_count, 0, 0..1);
    }
}
