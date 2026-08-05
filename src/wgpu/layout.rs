use super::binding::BindGroupLayout;

/// A bind group layout tagged with the `@group(N)` it occupies in the pipeline.
pub struct GroupLayout<'a> {
    pub group: u32,
    pub layout: &'a BindGroupLayout,
}

/// An owned bind group layout tagged with the `@group(N)` it occupies, for descriptors that
/// hold layouts by value.
pub struct OwnedGroupLayout {
    pub group: u32,
    pub layout: BindGroupLayout,
}

/// Assembles bind group layouts for a pipeline from explicit, group-tagged slots, rather
/// than an implicit position-based order. Panics if any group index in `0..=max_index` is
/// missing a layout, or if two slots claim the same index — both are almost always a
/// mistake that would otherwise show up later as an opaque wgpu shader validation error.
///
/// `slots` empty means zero bind groups, full stop — a material/compute
/// pass with `own_group: None` and no `extra_layouts` legitimately has no
/// bind group at all (e.g. a shader with no `@group` of its own), and that
/// must not be confused with "group 0 is missing," which is what
/// `max_group`'s `unwrap_or(0)` would otherwise imply once the loop below
/// runs against a one-slot-of-`None` array.
///
/// Returns raw `wgpu::BindGroupLayout` references (not the opaque wrapper) —
/// this is `pub(crate)` plumbing feeding directly into
/// `device.create_pipeline_layout`, not part of the public surface.
pub(crate) fn assemble_bind_group_layouts<'a>(
    label: Option<&str>,
    slots: Vec<GroupLayout<'a>>,
) -> Vec<Option<&'a wgpu::BindGroupLayout>> {
    if slots.is_empty() {
        return Vec::new();
    }

    let max_group = slots.iter().map(|s| s.group).max().unwrap();
    let mut assembled: Vec<Option<&wgpu::BindGroupLayout>> = vec![None; (max_group + 1) as usize];

    for GroupLayout { group, layout } in slots {
        let slot = &mut assembled[group as usize];
        if slot.is_some() {
            panic!(
                "bind group {group} assigned more than once building pipeline layout{}",
                label.map(|l| format!(" '{l}'")).unwrap_or_default()
            );
        }
        *slot = Some(layout.raw());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wgpu::binding::BindGroupLayoutBuilder;
    use crate::wgpu::test_util::with_device;

    fn empty_layout(device: &wgpu::Device) -> BindGroupLayout {
        BindGroupLayoutBuilder::new().build_raw(device)
    }

    #[test]
    fn no_slots_at_all_means_zero_bind_groups_not_a_missing_group_zero() {
        // Regression test: `slots` empty (a material/compute with
        // `own_group: None` and no `extra_layouts` — a shader with no
        // `@group` of its own) used to panic here, because
        // `max_group.unwrap_or(0)` created a single required-but-unfilled
        // slot for "group 0" out of nothing.
        let assembled = assemble_bind_group_layouts(None, vec![]);
        assert!(assembled.is_empty());
    }

    #[test]
    fn slots_out_of_order_still_assemble_into_group_order() {
        with_device!(device, _queue, {
            let a = empty_layout(&device);
            let b = empty_layout(&device);
            let c = empty_layout(&device);

            // Deliberately out of order — group 2 declared before group 0.
            let assembled = assemble_bind_group_layouts(
                None,
                vec![
                    GroupLayout { group: 2, layout: &c },
                    GroupLayout { group: 0, layout: &a },
                    GroupLayout { group: 1, layout: &b },
                ],
            );

            assert_eq!(assembled.len(), 3);
            assert!(std::ptr::eq(assembled[0].unwrap(), a.raw()));
            assert!(std::ptr::eq(assembled[1].unwrap(), b.raw()));
            assert!(std::ptr::eq(assembled[2].unwrap(), c.raw()));
        });
    }

    // Not `#[should_panic]`: these need a real device, which `with_device!`
    // skips gracefully (no panic at all) when none is available — a
    // `#[should_panic]` test would wrongly fail on exactly the machines
    // this is meant to tolerate. `catch_unwind` lets "skipped" and
    // "panicked as expected" both read as a passing test.
    #[test]
    fn two_slots_claiming_the_same_group_panics() {
        with_device!(device, _queue, {
            let a = empty_layout(&device);
            let b = empty_layout(&device);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                assemble_bind_group_layouts(
                    None,
                    vec![GroupLayout { group: 0, layout: &a }, GroupLayout { group: 0, layout: &b }],
                );
            }));
            assert!(result.is_err(), "expected a panic for a duplicate group index");
        });
    }

    #[test]
    fn a_gap_in_group_indices_panics() {
        with_device!(device, _queue, {
            let a = empty_layout(&device);
            let c = empty_layout(&device);
            // Group 1 is missing — groups must be contiguous from 0.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                assemble_bind_group_layouts(
                    None,
                    vec![GroupLayout { group: 0, layout: &a }, GroupLayout { group: 2, layout: &c }],
                );
            }));
            assert!(result.is_err(), "expected a panic for a gap in group indices");
        });
    }
}
