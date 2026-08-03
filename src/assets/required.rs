use std::{any::TypeId, collections::HashSet};

/// Tracks which resources are *provided* by plugins, so that a system
/// requiring one of them can be told apart from a system requiring a
/// resource nothing was ever going to insert.
///
/// Plugins call [`provides`](Self::provides) for each resource they
/// (eventually) insert. [`App::build`](crate::app::App::build) then walks
/// every registered system's [`System::requires`](crate::ecs::system::System::requires)
/// against this table — see [`App::validate_requirements`](crate::app::App::validate_requirements)
/// — panicking up front if any requirement has no provider, instead of
/// leaving it to surface as a runtime panic the first time that system's
/// stage runs.
pub(crate) struct RequiredResources {
    provided: HashSet<TypeId>,
}

impl RequiredResources {
    pub fn new() -> Self {
        Self {
            provided: Default::default(),
        }
    }

    /// Mark resource type `T` as provided by the calling plugin.
    pub fn provides<T: 'static>(&mut self) {
        self.provided.insert(TypeId::of::<T>());
    }

    /// Returns `true` if some plugin has declared (via [`provides`](Self::provides))
    /// that it eventually inserts this resource type.
    ///
    /// Used by [`App`](crate::app::App) to distinguish "this resource is
    /// legitimately still being constructed" (an async GPU backend, a
    /// [`LazyResource`](crate::assets::singleton_asset::LazyResource)) —
    /// where a system missing it should just wait — from a genuine
    /// oversight, where nothing was ever going to insert it.
    pub fn is_provided(&self, ty: TypeId) -> bool {
        self.provided.contains(&ty)
    }
}
