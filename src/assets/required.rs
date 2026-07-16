use std::{any::TypeId, collections::HashSet};

pub struct RequiredResources {
    provided: HashSet<TypeId>,
    required: Vec<(std::any::TypeId, &'static str)>,
}

impl RequiredResources {
    pub fn new() -> Self {
        Self {
            provided: Default::default(),
            required: Vec::new(),
        }
    }

    pub fn provides<T: 'static>(&mut self) {
        self.provided.insert(TypeId::of::<T>());
    }

    pub fn required<T: 'static>(&mut self, label: &'static str) {
        self.required.push((TypeId::of::<T>(), label));
    }

    pub fn validate(&self) {
        let missing: Vec<_> = self
            .required
            .iter()
            .filter(|(ty, _)| !self.provided.contains(ty))
            .map(|(_, label)| *label)
            .collect();

        if !missing.is_empty() {
            panic!(
                "Pebble startup resource validation failed - required resources with no provider registered:\n{}\n\nThis means a plugin depends on a resource that no other registered plugin provides.",
                missing
                    .iter()
                    .map(|m| format!(" - {m}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }
    }
}
