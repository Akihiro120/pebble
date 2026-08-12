# The Asset Pipeline and Handles

Every GPU resource in pebble — meshes, textures, materials, computes — follows the same unified CPU→GPU asset model. Understanding this once means every other rendering page in this book looks the same.

## The pattern

1. Build a CPU-side description (`Mesh::new(...)`, `Texture::from_file(...)`, `Material::new(...)`, ...).
2. Call `.build_asset(name, &mut assets)` to insert it into that type's `Assets<T>` and get back a `Handle<T>`.
3. Some time later — usually the very next `AssetSync` stage — the asset finishes uploading to the GPU. Until then, lookups on the handle return `None`.

```rust,ignore
fn setup(backend: Read<Backend>, mut meshes: Write<Assets<Mesh>>) {
    let handle: Handle<Mesh> = Mesh::new(vertices, indices).build_asset("player", &mut meshes);
    // stash `handle` in a component or resource
}

fn use_it(meshes: Read<Assets<Mesh>>, handle: Read<Handle<Mesh>>) {
    if let Some(gpu_mesh) = meshes.get(*handle) {
        // ready to draw
    }
}
```

`Handle<T>` is `Copy`, `Eq`, `Hash` — cheap to store in a component or use as a map key. A stale handle (its entry removed) just yields `None`, never panics.

## `Assets<T>`

- `insert(name, source)` — as above, via each type's `build_asset`.
- `get(handle)` — the GPU-side processed value, once ready.
- `get_source(handle)`/`get_source_mut(handle)` — the CPU-side source (e.g. a mesh's raw vertices, for building a collision shape from the same data).
- `is_ready(handle)` — whether the upload has finished.
- `mark_dirty(handle)` — re-queues an entry for re-upload, e.g. after mutating it via `get_source_mut`.
- `get_by_name`/`get_source_by_name`/`get_handle_by_name` — same lookups, keyed by the name string instead of a handle.
- `iter()` — every currently-loaded asset of this type, source data included, ready or not.

## Why uploads retry instead of failing

`Asset<B>::upload` returns `Option<Self::Processed>` — `None` just means "try again next tick," not an error. This is what lets you build a mesh before the GPU backend exists, or a material that depends on a shared bind group layout that hasn't been registered yet, without hand-rolled ordering: the asset system keeps retrying every `AssetSync` tick until every dependency (declared via `Asset::Deps`) is available, then uploads once and stops.

## Defining your own asset type

`AssetSource`/`Asset<B>` are the two traits every built-in type implements. The `asset!` macro is a pure syntax transform that writes both at once:

```rust,ignore
asset!(MyThing => GPUMyThing, |self, backend: &Backend| {
    Some(GPUMyThing { /* ... */ })
});

// or, with dependencies on other resources:
asset!(MyThing => GPUMyThing, deps: [SomePool], |self, backend: &Backend, deps| {
    Some(GPUMyThing { /* ... */ })
});
```

Then register it the same way every built-in type is registered:

```rust,ignore
app.add_plugin(AssetPlugin::<Backend, MyThing>::new())
```
