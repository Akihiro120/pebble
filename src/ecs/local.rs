use crate::ecs::{resources::Resources, system_param::SystemParam};

pub struct Local<'a, T: Default + hecs::Component> {
    data: &'a mut T,
}

impl<'a, T: Default + Send + Sync + 'static> std::ops::Deref for Local<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<'a, T: Default + Send + Sync + 'static> std::ops::DerefMut for Local<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

impl<T: Default + Send + Sync + 'static> SystemParam for Local<'_, T> {
    type Item<'w> = Local<'w, T>;
    type State = T;

    fn fetch<'w>(
        _world: &'w hecs::World,
        _resources: &'w Resources,
        state: &'w mut Self::State,
    ) -> Self::Item<'w> {
        Local { data: state }
    }
}
