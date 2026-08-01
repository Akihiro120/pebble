/// A bind group layout tagged with the `@group(N)` it occupies in the pipeline.
pub struct GroupLayout<'a> {
    pub group: u32,
    pub layout: &'a wgpu::BindGroupLayout,
}

/// An owned bind group layout tagged with the `@group(N)` it occupies, for descriptors that
/// hold layouts by value.
pub struct OwnedGroupLayout {
    pub group: u32,
    pub layout: wgpu::BindGroupLayout,
}

/// Assembles bind group layouts for a pipeline from explicit, group-tagged slots, rather
/// than an implicit position-based order. Panics if any group index in `0..=max_index` is
/// missing a layout, or if two slots claim the same index — both are almost always a
/// mistake that would otherwise show up later as an opaque wgpu shader validation error.
pub fn assemble_bind_group_layouts<'a>(
    label: Option<&str>,
    slots: Vec<GroupLayout<'a>>,
) -> Vec<Option<&'a wgpu::BindGroupLayout>> {
    let max_group = slots.iter().map(|s| s.group).max().unwrap_or(0);
    let mut assembled: Vec<Option<&wgpu::BindGroupLayout>> = vec![None; (max_group + 1) as usize];

    for GroupLayout { group, layout } in slots {
        let slot = &mut assembled[group as usize];
        if slot.is_some() {
            panic!(
                "bind group {group} assigned more than once building pipeline layout{}",
                label.map(|l| format!(" '{l}'")).unwrap_or_default()
            );
        }
        *slot = Some(layout);
    }

    for (i, slot) in assembled.iter().enumerate() {
        if slot.is_none() {
            panic!(
                "bind group {i} has no layout assigned building pipeline layout{} (groups must be contiguous from 0)",
                label.map(|l| format!(" '{l}'")).unwrap_or_default()
            );
        }
    }

    assembled
}
