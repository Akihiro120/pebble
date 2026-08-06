use super::binding::{BindGroupLayout, BindingEntry};

/// One `@group(N)` slot in a material/compute pipeline layout — position in the
/// [`Material::entries`](super::material::Material::entries)/
/// [`Compute::entries`](super::compute::Compute::entries) list *is* its `@group(N)` index, so
/// there's no separate group number to keep in sync with the shader by hand: the first element
/// occupies `@group(0)`, the second `@group(1)`, and so on.
pub enum GroupEntry {
    /// This material/compute's own bind group entries — built into a fresh layout
    /// internally, and the one group [`GPUMaterial`](super::material::GPUMaterial)/
    /// [`GPUCompute`](super::compute::GPUCompute) hand to a
    /// [`GPUBindingInstance`](super::instance::GPUBindingInstance) to bind concrete resources
    /// against at draw/dispatch time. At most one `Own` entry is allowed in a single
    /// `.entries(...)` list — `build_material`/`build_compute` panic on a second one, since
    /// there's only one instance-bindable group per material/compute.
    Own(Vec<BindingEntry>),
    /// An already-built layout occupying this position directly — a camera, lights, or any
    /// other external bind group layout, e.g. pulled from a [`GlobalLayoutPool`] via
    /// [`GlobalLayoutPool::get`].
    Layout(BindGroupLayout),
    /// A layout looked up by name in the [`GlobalLayoutPool`] resource, resolved lazily at
    /// *upload* time rather than when `.entries(...)` is called — so the material/compute
    /// doesn't need `name` to already be registered while it's being described, only by the
    /// time it actually uploads. If `name` isn't registered yet, upload quietly returns `None`
    /// and retries next tick, the same "not ready" convention as any other `Deps`. Prefer this
    /// over resolving `GlobalLayoutPool::get` yourself and wrapping the result in
    /// [`Layout`](Self::Layout) — that requires the pool to already have `name` at the point
    /// you build the descriptor, which is a race the `setup` system authoring a material has
    /// no natural way to wait out on its own.
    Global(&'static str),
}

/// A named pool of bind group layouts shared across materials/compute passes — register a
/// layout once (e.g. a camera's, under `"camera"`) as soon as it exists, then anywhere a
/// material/compute wants it, pull it with [`get`](Self::get) and wrap it in
/// [`GroupEntry::Layout`] at whatever position that material/compute's shader declares it.
///
/// [`WGPUPlugin`](super::backend::WGPUPlugin) inserts an empty pool as a resource, so it's
/// always there from the start — grab it with `Res<GlobalLayoutPool>`/
/// `ResMut<GlobalLayoutPool>` rather than constructing your own; a `LazyResource` that builds a
/// shared layout (a camera, lights, ...) registers it into that same pool from its own
/// `construct` (or a follow-up system, once it has `ResMut<GlobalLayoutPool>` alongside it) —
/// there's no separate "finished pool" step, entries just accumulate as their sources become
/// ready.
#[derive(Default)]
pub struct GlobalLayoutPool {
    entries: std::collections::HashMap<&'static str, BindGroupLayout>,
}

impl GlobalLayoutPool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `layout` under `name`. Panics if `name` is already registered — almost always
    /// two sources registering under the same name by mistake, not an intentional overwrite.
    pub fn register(&mut self, name: &'static str, layout: BindGroupLayout) {
        if self.entries.insert(name, layout).is_some() {
            panic!("global layout pool: '{name}' is already registered");
        }
    }

    /// The layout registered under `name`, if any — clone it into a [`GroupEntry::Layout`] at
    /// whatever position your shader declares it. `None` if nothing has registered under that
    /// name (yet, or ever — a typo'd name and "not built yet" look the same from here, so
    /// callers with a hard requirement on a given global should treat a miss as "not ready"
    /// the same way any other `Option`-returning lookup in this engine does). Reach for
    /// [`GroupEntry::Global`] instead of calling this directly wherever possible — it defers
    /// this same lookup to upload time, so `name` doesn't need to be registered yet at the
    /// point a material/compute is described.
    pub fn get(&self, name: &str) -> Option<BindGroupLayout> {
        self.entries.get(name).cloned()
    }

    /// Same lookup as [`get`](Self::get) without cloning — used internally by
    /// [`assemble_group_layouts`] to resolve [`GroupEntry::Global`] against a borrowed pool.
    pub(crate) fn get_ref(&self, name: &str) -> Option<&BindGroupLayout> {
        self.entries.get(name)
    }
}

/// Which pipeline kind a panic message from [`find_own_entries`] is describing — only used
/// for wording those messages (`Material`'s bind group entries are validated differently than
/// `Compute`'s, but both funnel through the same shared function).
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

/// Finds the single [`GroupEntry::Own`] in `groups`, if any, returning its entries (or `&[]`
/// if there isn't one — a shader with no `@group` of its own). Panics if there's more than
/// one — a material/compute can only expose one concrete bind group for a
/// [`GPUBindingInstance`](super::instance::GPUBindingInstance) to bind resources against, so
/// at most one position in `.entries(...)` may be `Own`.
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

/// Assembles the ordered pipeline-layout slots from `groups` — position in `groups` is the
/// `@group(N)` index, with `own_layout` (built by the caller from
/// [`find_own_entries`]'s result) filling in wherever [`GroupEntry::Own`] appeared, and `pool`
/// resolving wherever [`GroupEntry::Global`] appeared.
///
/// Returns `None` — not a panic — if any `GroupEntry::Global` name isn't registered in `pool`
/// yet: unlike every other failure mode here, a missing global is a timing issue (the
/// `LazyResource` that registers it hasn't run yet), not a caller mistake, so it gets the same
/// "not ready, retry next tick" treatment as any other unmet `Deps`.
///
/// Panics if `groups` needs more bind groups than `max_bind_groups` allows — `wgpu` guarantees
/// only 4 (`@group(0..=3)`) unless a device explicitly requests/supports more, so this is the
/// difference between a clear message here (the actual limit and how many groups were
/// requested) and an opaque wgpu validation panic at pipeline-layout creation. This is the
/// reason to only list the groups a shader actually declares in `.entries(...)` — a
/// [`GlobalLayoutPool`] registration you don't need is one you shouldn't reach for.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wgpu::binding::BindGroupLayoutBuilder;
    use crate::wgpu::test_util::with_device;

    fn empty_layout(device: &wgpu::Device) -> BindGroupLayout {
        BindGroupLayoutBuilder::new().build_raw(device)
    }

    #[test]
    fn global_layout_pool_get_round_trips_through_register() {
        with_device!(device, _queue, {
            let mut pool = GlobalLayoutPool::new();
            pool.register("camera", empty_layout(&device));

            assert!(pool.get("camera").is_some());
            assert!(pool.get("missing").is_none());
        });
    }

    #[test]
    fn global_layout_pool_panics_on_duplicate_name() {
        with_device!(device, _queue, {
            let mut pool = GlobalLayoutPool::new();
            pool.register("camera", empty_layout(&device));
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                pool.register("camera", empty_layout(&device));
            }));
            assert!(result.is_err(), "expected a panic for a duplicate name registered in the pool");
        });
    }

    #[test]
    fn find_own_entries_returns_empty_slice_when_there_is_no_own_group() {
        let entries = find_own_entries(None, PipelineKind::Material, &[]);
        assert!(entries.is_empty());
    }

    #[test]
    fn find_own_entries_panics_on_more_than_one_own_group() {
        with_device!(device, _queue, {
            let groups =
                vec![GroupEntry::Own(vec![]), GroupEntry::Layout(empty_layout(&device)), GroupEntry::Own(vec![])];
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                find_own_entries(None, PipelineKind::Material, &groups);
            }));
            assert!(result.is_err(), "expected a panic for more than one GroupEntry::Own");
        });
    }

    #[test]
    fn assemble_group_layouts_orders_slots_by_position() {
        with_device!(device, _queue, {
            let own = empty_layout(&device);
            let a = empty_layout(&device);
            let b = empty_layout(&device);
            let groups =
                vec![GroupEntry::Layout(a), GroupEntry::Own(vec![]), GroupEntry::Layout(b)];
            let pool = GlobalLayoutPool::new();

            let assembled = assemble_group_layouts(None, &groups, &own, &pool, 4).unwrap();

            assert_eq!(assembled.len(), 3);
            let GroupEntry::Layout(a) = &groups[0] else { unreachable!() };
            let GroupEntry::Layout(b) = &groups[2] else { unreachable!() };
            assert!(std::ptr::eq(assembled[0].unwrap(), a.raw()));
            assert!(std::ptr::eq(assembled[1].unwrap(), own.raw()));
            assert!(std::ptr::eq(assembled[2].unwrap(), b.raw()));
        });
    }

    #[test]
    fn assemble_group_layouts_resolves_global_entries_from_the_pool() {
        with_device!(device, _queue, {
            let own = empty_layout(&device);
            let mut pool = GlobalLayoutPool::new();
            pool.register("camera", empty_layout(&device));
            let groups = vec![GroupEntry::Own(vec![]), GroupEntry::Global("camera")];

            let assembled = assemble_group_layouts(None, &groups, &own, &pool, 4).unwrap();

            assert_eq!(assembled.len(), 2);
            assert!(std::ptr::eq(assembled[1].unwrap(), pool.get_ref("camera").unwrap().raw()));
        });
    }

    #[test]
    fn assemble_group_layouts_returns_none_for_an_unregistered_global() {
        with_device!(device, _queue, {
            let own = empty_layout(&device);
            let pool = GlobalLayoutPool::new(); // "camera" never registered
            let groups = vec![GroupEntry::Global("camera")];

            let assembled = assemble_group_layouts(None, &groups, &own, &pool, 4);

            assert!(assembled.is_none(), "expected None (not ready), not a panic, for an unresolved global");
        });
    }

    #[test]
    fn exceeding_max_bind_groups_panics() {
        with_device!(device, _queue, {
            let own = empty_layout(&device);
            let groups = vec![GroupEntry::Layout(empty_layout(&device)), GroupEntry::Layout(empty_layout(&device))];
            let pool = GlobalLayoutPool::new();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                assemble_group_layouts(None, &groups, &own, &pool, 1);
            }));
            assert!(result.is_err(), "expected a panic for exceeding max_bind_groups");
        });
    }
}
