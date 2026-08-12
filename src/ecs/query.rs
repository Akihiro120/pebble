use crate::ecs::{resources::Resources, system_param::SystemParam};

/// Fetches components matching `Q` from every entity that has them. No
/// `hecs::*` type appears in its own public methods.
pub struct Query<'a, Q: hecs::Query> {
    world: &'a hecs::World,
    borrow: hecs::QueryBorrow<'a, Q>,
    scratch: Option<hecs::QueryOne<'a, Q>>,
}

impl<'a, Q: hecs::Query> IntoIterator for &'a mut Query<'a, Q> {
    type Item = Q::Item<'a>;
    type IntoIter = hecs::QueryIter<'a, Q>;

    fn into_iter(self) -> Self::IntoIter {
        (&mut self.borrow).into_iter()
    }
}

impl<'a, Q: hecs::Query> Query<'a, Q> {
    /// Iterates every matching entity's components.
    pub fn iter(&mut self) -> impl Iterator<Item = Q::Item<'_>> {
        self.borrow.iter()
    }

    /// Fetches one known entity directly, without scanning the rest of the
    /// query. `None` if it doesn't exist or doesn't match `Q`.
    pub fn get(&mut self, entity: hecs::Entity) -> Option<Q::Item<'_>> {
        self.scratch = Some(self.world.query_one::<Q>(entity));
        self.scratch.as_mut().unwrap().get().ok()
    }

    /// Narrows to entities that also have component(s) `R`, without `R`
    /// joining the yielded items. Chainable with `.without::<S>()`.
    pub fn with<R: hecs::Query>(self) -> Query<'a, hecs::With<Q, R>> {
        Query {
            world: self.world,
            borrow: self.borrow.with::<R>(),
            scratch: None,
        }
    }

    /// Narrows to entities that don't have component(s) `R`.
    pub fn without<R: hecs::Query>(self) -> Query<'a, hecs::Without<Q, R>> {
        Query {
            world: self.world,
            borrow: self.borrow.without::<R>(),
            scratch: None,
        }
    }

    /// Expects exactly one matching entity (the player, the active
    /// camera). Panics otherwise — use [`get_single`](Self::get_single) if
    /// that's not guaranteed.
    pub fn single(&mut self) -> Q::Item<'_> {
        self.get_single()
            .expect("Query::single: expected exactly one matching entity")
    }

    /// Same as [`single`](Self::single), but `None` instead of panicking
    /// when there isn't exactly one match.
    pub fn get_single(&mut self) -> Option<Q::Item<'_>> {
        let mut iter = self.borrow.iter();
        let first = iter.next()?;
        if iter.next().is_some() {
            return None;
        }
        Some(first)
    }
}

impl<Q> SystemParam for Query<'static, Q>
where
    Q: hecs::Query + 'static,
{
    type Item<'a> = Query<'a, Q>;
    type State = ();

    fn fetch<'a>(
        world: &'a hecs::World,
        _resources: &'a Resources,
        _state: &'a mut Self::State,
    ) -> Self::Item<'a> {
        Query {
            world,
            borrow: world.query::<Q>(),
            scratch: None,
        }
    }
}
