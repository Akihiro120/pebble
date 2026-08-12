use crate::graphics::pipeline::binding::{BindGroupLayout, BindingEntry, BindingKind};

pub enum GroupEntry {
    Own(Vec<BindingEntry>),
    Layout(BindGroupLayout),
    Global(&'static str),
}

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
}

#[derive(Default)]
pub struct GlobalLayoutPool {
    entries: std::collections::HashMap<&'static str, BindGroupLayout>,
}

impl GlobalLayoutPool {
    pub fn new() -> Self {
        Self::default()
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_own_entries_returns_empty_slice_when_there_is_no_own_group() {
        let entries = find_own_entries(None, PipelineKind::Material, &[]);
        assert!(entries.is_empty());
    }
}
