use std::{any::TypeId, collections::HashSet};

/// Tracks which resources are *provided* by plugins and which are *required*,
/// validating at startup that every requirement has a corresponding provider.
///
/// Plugins call [`provides`] for each resource they insert and [`required`] for
/// each resource they depend on. [`App::build`](crate::app::App::build) calls
/// [`validate`] after all plugins have been built, panicking with a clear
/// message if any requirement is unmet.
pub(crate) struct RequiredResources {
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

    /// Mark resource type `T` as provided by the calling plugin.
    pub fn provides<T: 'static>(&mut self) {
        self.provided.insert(TypeId::of::<T>());
    }

    /// Declare that resource type `T` is required, with `label` used in the
    /// error message if it is absent at startup.
    pub fn required<T: 'static>(&mut self, label: &'static str) {
        self.required.push((TypeId::of::<T>(), label));
    }

    /// Panic if any declared requirement has no matching provider.
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
