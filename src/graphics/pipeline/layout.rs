use std::cell::RefCell;
use std::collections::HashMap;

use crate::graphics::{
    pipeline::{
        binding::{BindGroupLayout, BindingEntry, BindingKind},
        compute::ComputePipeline,
        material::RenderPipeline,
    },
    types::{
        Face, PolygonMode,
        pipeline_state::{ColorTargetState, DepthStencilState, VertexBufferLayout},
    },
};

/// One bind group beyond a [`Material`](super::material::Material)/[`Compute`](super::compute::Compute)'s
/// own (group 0, built automatically from its `.texture(...)`/`.sampler(...)`/etc.
/// calls) — passed to `.with_extra_group(...)` for group 1 and up: a
/// pre-built [`BindGroupLayout`], or a name looked up in the
/// [`GlobalLayoutPool`] (for layouts shared across pipelines). `Own` also
/// exists for the rare case of assembling one by hand instead.
#[derive(Clone)]
pub enum GroupEntry {
    Own(Vec<BindingEntry>),
    Layout(BindGroupLayout),
    Global(&'static str),
}

/// Builds the [`GroupEntry::Own`] list for a pipeline's own bind group —
/// auto-increments binding indices unless you use
/// [`with_entry_at`](Self::with_entry_at) to pin one explicitly.
#[derive(Default)]
pub struct OwnEntriesBuilder {
    entries: Vec<BindingEntry>,
    next_binding: u32,
}

impl OwnEntriesBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_entry(self, name: &'static str, kind: BindingKind) -> Self {
        let binding = self.next_binding;
        self.with_entry_at(name, binding, kind)
    }

    pub fn with_entry_at(mut self, name: &'static str, binding: u32, kind: BindingKind) -> Self {
        self.entries.push(BindingEntry { name, binding, kind });
        self.next_binding = self.next_binding.max(binding + 1);
        self
    }

    pub fn build(self) -> GroupEntry {
        GroupEntry::Own(self.entries)
    }

    /// Peeks at the entries accumulated so far, without consuming the
    /// builder — lets `Material`/`Compute` read their own accumulated
    /// entries repeatedly (upload runs on `&self`) instead of only once via
    /// [`build`](Self::build).
    pub(crate) fn entries(&self) -> &[BindingEntry] {
        &self.entries
    }
}

/// A registry of named bind group layouts, inserted as a resource by
/// [`BuiltinAssetsPlugin`](crate::graphics::BuiltinAssetsPlugin) — lets
/// unrelated materials/computes share one layout via [`GroupEntry::Global`]
/// instead of each declaring their own.
#[derive(Default)]
pub struct GlobalLayoutPool {
    entries: std::collections::HashMap<&'static str, BindGroupLayout>,
}

impl GlobalLayoutPool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a layout under `name`. Panics if `name` is already registered.
    pub fn register(&mut self, name: &'static str, layout: BindGroupLayout) {
        if self.entries.insert(name, layout).is_some() {
            panic!("global layout pool: '{name}' is already registered");
        }
    }

    pub fn get(&self, name: &str) -> Option<BindGroupLayout> {
        self.entries.get(name).cloned()
    }

    pub(crate) fn get_ref(&self, name: &str) -> Option<&BindGroupLayout> {
        self.entries.get(name)
    }
}

#[derive(Clone, Copy)]
pub(crate) enum PipelineKind {
    Material,
    Compute,
}

impl std::fmt::Display for PipelineKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            PipelineKind::Material => "material",
            PipelineKind::Compute => "compute pass",
        })
    }
}

pub(crate) fn find_own_entries<'a>(
    label: Option<&str>,
    kind: PipelineKind,
    groups: &'a [GroupEntry],
) -> &'a [BindingEntry] {
    let mut found: Option<&[BindingEntry]> = None;
    for g in groups {
        if let GroupEntry::Own(entries) = g {
            if found.is_some() {
                panic!(
                    "{kind}{}: more than one GroupEntry::Own(...) in .entries(...) — a {kind} \
                     can only have one group of its own bind group entries",
                    label.map(|l| format!(" '{l}'")).unwrap_or_default()
                );
            }
            found = Some(entries);
        }
    }
    found.unwrap_or(&[])
}

pub(crate) fn assemble_group_layouts<'a>(
    label: Option<&str>,
    groups: &'a [GroupEntry],
    own_layout: &'a BindGroupLayout,
    pool: &'a GlobalLayoutPool,
    max_bind_groups: u32,
) -> Option<Vec<Option<&'a wgpu::BindGroupLayout>>> {
    if groups.len() as u32 > max_bind_groups {
        panic!(
            "pipeline layout{} needs {} bind groups, but this device only supports \
             {max_bind_groups} — trim .entries(...) to only the groups actually used",
            label.map(|l| format!(" '{l}'")).unwrap_or_default(),
            groups.len(),
        );
    }

    groups
        .iter()
        .map(|g| {
            let layout = match g {
                GroupEntry::Own(_) => own_layout,
                GroupEntry::Layout(l) => l,
                GroupEntry::Global(name) => pool.get_ref(name)?,
            };
            Some(Some(layout.raw()))
        })
        .collect()
}

/// Structural key for one [`GroupEntry`] in a pipeline-cache key. No variant
/// for `GroupEntry::Layout(_)` — see [`group_keys`].
#[derive(PartialEq, Eq, Hash)]
enum GroupKey {
    Own(Vec<BindingEntry>),
    Global(&'static str),
}

/// Builds the cacheable key for `groups`, or `None` if any entry is a
/// `GroupEntry::Layout(_)` (an inline pre-built layout has no meaningful
/// structural equality to key on) — a `Material`/`Compute` using one always
/// compiles its own pipeline, same as every pipeline did before caching
/// existed. The common case (`Own`/`Global` only, which is what sharing a
/// shader across many uniform-value combinations actually uses) is fully
/// cacheable.
fn group_keys(groups: &[GroupEntry]) -> Option<Vec<GroupKey>> {
    groups
        .iter()
        .map(|g| match g {
            GroupEntry::Own(entries) => Some(GroupKey::Own(entries.clone())),
            GroupEntry::Global(name) => Some(GroupKey::Global(name)),
            GroupEntry::Layout(_) => None,
        })
        .collect()
}

/// Every field of a [`Material`](super::material::Material) that actually
/// affects the compiled `wgpu::RenderPipeline`/pipeline layout — `label` is
/// deliberately excluded (it's a debug name, not pipeline-affecting).
/// `targets` must already be resolved (`TargetsSpec::resolve`) before
/// building this, so `Material::standard`'s surface-format marker keys on
/// the real resolved format, not the marker itself.
#[derive(PartialEq, Eq, Hash)]
pub(crate) struct MaterialPipelineKey {
    pub shader_source: &'static str,
    pub vertex_entry: Option<&'static str>,
    pub fragment_entry: Option<&'static str>,
    pub vertex_layouts: Vec<VertexBufferLayout>,
    pub cull_mode: Option<Face>,
    pub depth: Option<DepthStencilState>,
    pub targets: Vec<ColorTargetState>,
    pub polygon_mode: PolygonMode,
    pub sample_count: u32,
    groups: Vec<GroupKey>,
}

impl MaterialPipelineKey {
    /// `None` if `groups` contains a `GroupEntry::Layout(_)` — see [`group_keys`].
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        shader_source: &'static str,
        vertex_entry: Option<&'static str>,
        fragment_entry: Option<&'static str>,
        vertex_layouts: Vec<VertexBufferLayout>,
        cull_mode: Option<Face>,
        depth: Option<DepthStencilState>,
        targets: Vec<ColorTargetState>,
        polygon_mode: PolygonMode,
        sample_count: u32,
        groups: &[GroupEntry],
    ) -> Option<Self> {
        Some(Self {
            shader_source,
            vertex_entry,
            fragment_entry,
            vertex_layouts,
            cull_mode,
            depth,
            targets,
            polygon_mode,
            sample_count,
            groups: group_keys(groups)?,
        })
    }
}

/// Deduplicates compiled [`Material`](super::material::Material) pipelines —
/// many `Material`s sharing the same shader/fixed-function state (e.g.
/// several uniform-value combinations against one shader) compile once and
/// share the result, instead of each getting its own `wgpu::RenderPipeline`.
/// Registered as a resource by
/// [`BuiltinAssetsPlugin`](crate::graphics::BuiltinAssetsPlugin).
/// `RefCell`-backed so it can be mutated from inside `Material::upload`,
/// which only ever gets a `Read<'_, _>` borrow of its dependencies (see
/// [`Dependencies`](crate::assets::deps::Dependencies) — there's no
/// `Write<T>` dependency support).
#[derive(Default)]
pub struct MaterialPipelineCache {
    entries: RefCell<HashMap<MaterialPipelineKey, (RenderPipeline, BindGroupLayout)>>,
}

impl MaterialPipelineCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// The cached `(pipeline, layout)` for `key`, compiling and caching it
    /// via `compile` on a miss. `key` of `None` (a `Material` using
    /// `GroupEntry::Layout(_)`) always compiles, never caches.
    pub(crate) fn get_or_compile(
        &self,
        key: Option<MaterialPipelineKey>,
        compile: impl FnOnce() -> Option<(RenderPipeline, BindGroupLayout)>,
    ) -> Option<(RenderPipeline, BindGroupLayout)> {
        let Some(key) = key else { return compile() };
        if let Some(cached) = self.entries.borrow().get(&key) {
            return Some(cached.clone());
        }
        let built = compile()?;
        self.entries.borrow_mut().insert(key, built.clone());
        Some(built)
    }
}

/// Same as [`MaterialPipelineKey`], for [`Compute`](super::compute::Compute)
/// — no vertex/fragment/target/depth state, just what a compute pipeline
/// actually has.
#[derive(PartialEq, Eq, Hash)]
pub(crate) struct ComputePipelineKey {
    pub shader_source: &'static str,
    pub entry_point: Option<&'static str>,
    groups: Vec<GroupKey>,
}

impl ComputePipelineKey {
    /// `None` if `groups` contains a `GroupEntry::Layout(_)` — see [`group_keys`].
    pub fn new(shader_source: &'static str, entry_point: Option<&'static str>, groups: &[GroupEntry]) -> Option<Self> {
        Some(Self { shader_source, entry_point, groups: group_keys(groups)? })
    }
}

/// Same as [`MaterialPipelineCache`], for [`Compute`](super::compute::Compute).
#[derive(Default)]
pub struct ComputePipelineCache {
    entries: RefCell<HashMap<ComputePipelineKey, (ComputePipeline, BindGroupLayout)>>,
}

impl ComputePipelineCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn get_or_compile(
        &self,
        key: Option<ComputePipelineKey>,
        compile: impl FnOnce() -> Option<(ComputePipeline, BindGroupLayout)>,
    ) -> Option<(ComputePipeline, BindGroupLayout)> {
        let Some(key) = key else { return compile() };
        if let Some(cached) = self.entries.borrow().get(&key) {
            return Some(cached.clone());
        }
        let built = compile()?;
        self.entries.borrow_mut().insert(key, built.clone());
        Some(built)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_own_entries_returns_empty_slice_when_there_is_no_own_group() {
        let entries = find_own_entries(None, PipelineKind::Material, &[]);
        assert!(entries.is_empty());
    }
}
