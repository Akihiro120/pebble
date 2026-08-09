use std::collections::HashMap;

use crate::{
    app::{App, SystemStage},
    assets::{handle::Handle, storage::RawAssetHandle},
    ecs::{plugin::Plugin, system::{Commands, Local, Query, Res, ResMut}},
};

use super::{
    backend::WGPUBackend,
    binding::{BindGroupLayout, BindGroupLayoutBuilder, BindingKind},
    buffer::Buffer,
    buffers::{BindGroup, BindGroupBuilder, BufferBuilder},
    flags::ShaderStages,
    layout::GlobalLayoutPool,
    material::Material,
    player::AnimationPlayer,
    skinned_mesh::SkinnedMesh,
};

// ── internal ──────────────────────────────────────────────────────────────────

struct SkinningBatch {
    matrices:    Buffer,   // storage: capacity * joint_count * mat4
    _info:       Buffer,   // uniform: SkinningInfo { joint_count: u32, _pad: [u32; 3] }
    bind_group:  BindGroup,
    joint_count: u32,
    capacity:    u32,
}

impl SkinningBatch {
    fn new(backend: &WGPUBackend, layout: &BindGroupLayout, joint_count: u32, capacity: u32) -> Self {
        let matrices = BufferBuilder::empty((capacity as u64) * (joint_count as u64) * 64)
            .with_label("pebble_skinning:joint_matrices")
            .with_storage()
            .build(backend);
        let info = BufferBuilder::with_data(bytemuck::cast_slice(&[joint_count, 0u32, 0u32, 0u32]))
            .with_label("pebble_skinning:info")
            .with_uniform()
            .build(backend);
        let bind_group = BindGroupBuilder::new(layout)
            .with_buffer(&matrices)
            .with_buffer(&info)
            .build(backend);
        Self { matrices, _info: info, bind_group, joint_count, capacity }
    }
}

// ── public ────────────────────────────────────────────────────────────────────

/// Holds one GPU buffer pair per unique `(material, mesh)` batch — one per
/// skeleton type in practice. Retrieve the bind group for a batch from your
/// render system via [`bind_group`](Self::bind_group).
///
/// WGSL layout for `GroupEntry::Global("pebble_skinning")`:
/// ```wgsl
/// struct SkinningInfo { joint_count: u32 }
/// @group(N) @binding(0) var<storage, read> joint_matrices: array<mat4x4<f32>>;
/// @group(N) @binding(1) var<uniform>        skin_info:     SkinningInfo;
/// // in vs_main: let base = instance_index * skin_info.joint_count;
/// ```
pub struct SkinnedBatchRenderer {
    layout:  BindGroupLayout,
    batches: HashMap<(RawAssetHandle, RawAssetHandle), SkinningBatch>,
}

impl SkinnedBatchRenderer {
    fn new(backend: &WGPUBackend) -> Self {
        let layout = BindGroupLayoutBuilder::new()
            .with_label("pebble_skinning")
            .with_entry("joint_matrices", 0, BindingKind::storage_buffer_read_only(ShaderStages::VERTEX))
            .with_entry("skin_info",      1, BindingKind::uniform_buffer(ShaderStages::VERTEX))
            .build(backend);
        Self { layout, batches: HashMap::new() }
    }

    /// The bind group for the `(material, mesh)` batch — pass to
    /// `set_bind_group` immediately before `draw_indexed(0..index_count, 0, 0..instance_count)`.
    pub fn bind_group(&self, material: RawAssetHandle, mesh: RawAssetHandle) -> Option<&BindGroup> {
        self.batches.get(&(material, mesh)).map(|b| &b.bind_group)
    }

    fn prepare(&mut self, key: (RawAssetHandle, RawAssetHandle), joint_count: u32, needed: u32, backend: &WGPUBackend) {
        let layout = self.layout.clone();
        let batch = self.batches.entry(key).or_insert_with(|| {
            SkinningBatch::new(backend, &layout, joint_count, needed.max(256))
        });
        if needed > batch.capacity {
            *batch = SkinningBatch::new(backend, &layout, batch.joint_count, (batch.capacity * 2).max(needed));
        }
    }
}

/// One entry per unique `(material, mesh)` pair — produced by
/// [`batch_skinned_entities`] each `PreRender` tick. Iterate in your render
/// system alongside [`SkinnedBatchRenderer`] to drive draw calls.
pub struct SkinnedBatchUnit {
    pub material:       RawAssetHandle,
    pub mesh:           RawAssetHandle,
    pub instance_count: u32,
}

/// The frame output of the skinning batch system.
///
/// ```ignore
/// for batch in storage.batches.iter() {
///     let Some(bind_group) = renderer.bind_group(batch.material, batch.mesh) else { continue };
///     pass.set_bind_group(0, bind_group, &[]);
///     pass.draw_indexed(0..mesh.index_count, 0, 0..batch.instance_count);
/// }
/// ```
#[derive(Default)]
pub struct SkinnedBatchStorage {
    pub batches: Vec<SkinnedBatchUnit>,
}

// ── systems ───────────────────────────────────────────────────────────────────

fn init_skinned_batching(
    mut commands: Commands,
    backend:      Res<WGPUBackend>,
    mut pool:     ResMut<GlobalLayoutPool>,
) -> Option<()> {
    let renderer = SkinnedBatchRenderer::new(&backend);
    pool.register("pebble_skinning", renderer.layout.clone());
    commands.insert_resource(renderer);
    commands.insert_resource(SkinnedBatchStorage::default());
    Some(())
}

struct GroupData {
    joint_count: u32,
    matrices:    Vec<glam::Mat4>, // flat: entity0 joints, entity1 joints, ...
    count:       u32,
}

fn batch_skinned_entities(
    backend:      Res<WGPUBackend>,
    mut renderer: ResMut<SkinnedBatchRenderer>,
    mut storage:  ResMut<SkinnedBatchStorage>,
    mut query:    Query<(&Handle<Material>, &Handle<SkinnedMesh>, &AnimationPlayer)>,
    mut groups:   Local<HashMap<(RawAssetHandle, RawAssetHandle), GroupData>>,
) {
    storage.batches.clear();
    groups.clear();

    for (mat, mesh, player) in query.iter() {
        let entry = groups.entry((mat.id, mesh.id)).or_insert_with(|| GroupData {
            joint_count: player.joint_count() as u32,
            matrices:    Vec::new(),
            count:       0,
        });
        entry.matrices.extend(player.compute_matrices());
        entry.count += 1;
    }

    for ((mat_id, mesh_id), group) in groups.iter() {
        renderer.prepare((*mat_id, *mesh_id), group.joint_count, group.count, &backend);
        let batch = renderer.batches.get(&(*mat_id, *mesh_id)).unwrap();
        batch.matrices.write(bytemuck::cast_slice(&group.matrices));
        storage.batches.push(SkinnedBatchUnit {
            material:       *mat_id,
            mesh:           *mesh_id,
            instance_count: group.count,
        });
    }
}

/// Registers the `"pebble_skinning"` bind group layout, creates
/// [`SkinnedBatchRenderer`] and [`SkinnedBatchStorage`] resources, and runs
/// [`batch_skinned_entities`] in [`SystemStage::PreRender`].
///
/// The plugin does **not** advance animation time — add your own system that
/// calls [`AnimationPlayer::advance`](super::player::AnimationPlayer::advance).
pub struct SkinnedBatchingPlugin;

impl Plugin for SkinnedBatchingPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(SystemStage::Startup, init_skinned_batching);
        app.add_system(SystemStage::PreRender, batch_skinned_entities);
    }
}
