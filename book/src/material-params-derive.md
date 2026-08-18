# Material/Compute Params (derive)

`#[derive(MaterialParams)]`/`#[derive(ComputeParams)]` (from `pebble-derive`, re-exported from `Material`/`Compute`'s own modules) turn a plain struct's fields into the `.texture(...)`/`.uniform_value(...)`/etc. chain from [Materials](./materials.md)/[Compute Pipelines](./compute-pipelines.md) — so the struct's shape *is* the bind group, instead of a chain you write and keep in sync with it by hand:

```rust,ignore
use pebble::graphics::pipeline::material::{Material, MaterialParams};

#[derive(MaterialParams)]
struct EnemyMaterialParams {
    #[uniform(0)]
    tint: Vec4,
    #[uniform(0)]
    emissive: f32,          // same index as `tint` → packed into one generated buffer together
    #[texture(1)]
    albedo: Handle<Texture>,
    #[sampler(2)]
    sampler: SamplerKind,
}

let mat = EnemyMaterialParams { tint: RED, emissive: 2.0, albedo, sampler: SamplerKind::LinearRepeat }
    .into_material(Material::standard(ENEMY_SHADER))
    .build_asset("enemy_red", &mut materials);
```

The derive generates one method, `into_material(self, base: Material, ...) -> Material` (`into_compute`/`Compute` for `#[derive(ComputeParams)]`) — it doesn't replace `Material`/`Compute`, it just writes the builder chain for you and hands back the same `Material` you'd have built by hand, ready for `.build_asset(...)`.

## Field attributes

`#[uniform(N)]`, `#[storage(N)]`, `#[texture(N)]`, `#[texture_array(N)]`, `#[cubemap(N)]`, `#[sampler(N)]` — every field needs exactly one. `N` is the real WGSL `@binding(N)` index, same as `.with_entry_at`'s.

**Grouping.** Several `#[uniform(N)]`/`#[storage(N)]` fields sharing the same `N` pack into one generated buffer together (named after the first field in the group) — matching how a WGSL `uniform`/`storage` block is one binding no matter how many members it has. Every other kind needs a binding to itself; two fields sharing a `#[texture(N)]`/etc. index is a compile error.

**Type checking.** `#[texture(N)]`/`#[texture_array(N)]`/`#[cubemap(N)]`/`#[sampler(N)]` fields are checked against the shape they're expected to be, on a best-effort basis: a field type that's recognizably wrong (`#[texture(1)] foo: Handle<Cubemap>`) is a clear compile error pointing at the field; anything not confidently recognized (a type alias, an unusual path) is silently left to rustc's own type error at the generated call site, same as if the check didn't exist. `#[uniform]`/`#[storage]` fields can be any type implementing `encase::ShaderType` (see [Materials](./materials.md#typed-uniforms-with-encase)) — there's no fixed shape to check there.

## Visibility

Defaults to `FRAGMENT` for `MaterialParams`, always exactly `COMPUTE` for `ComputeParams` (a compute bind group entry can't be anything else — `#[derive(ComputeParams)]` rejects any override attempt with a compile error). Override per field on a `MaterialParams` struct with a second attribute argument:

```rust,ignore
#[derive(MaterialParams)]
struct SkinnedMaterialParams {
    #[uniform(0, vertex)]
    joint_matrices: JointMatrices,
    #[texture(1, vertex_fragment)]
    displacement_map: Handle<Texture>,
    #[texture(2)]   // no override — stays FRAGMENT
    albedo: Handle<Texture>,
}
```

`vertex`, `fragment`, or `vertex_fragment`. Every field sharing a grouped `#[uniform(N)]`/`#[storage(N)]` index must agree on the same visibility (explicit or all-default) — a compile error otherwise.

## Optional textures

A `#[texture(N)]`/`#[texture_array(N)]`/`#[cubemap(N)]` field typed `Option<Handle<T>>` instead of `Handle<T>` binds a fallback texture when the value is `None` — the WGSL binding always exists regardless of whether a given instance has a value, so `into_material`/`into_compute` gains one extra `{field}_fallback: Handle<T>` parameter per optional field:

```rust,ignore
#[derive(MaterialParams)]
struct EnemyMaterialParams {
    #[texture(0)]
    albedo: Option<Handle<Texture>>,
}

let mat = EnemyMaterialParams { albedo: enemy.custom_skin }   // Option<Handle<Texture>>
    .into_material(Material::standard(SHADER), default_skin_texture)   // used only if albedo is None
    .build_asset("enemy", &mut materials);
```

## `#[layout(...)]`: a bind group beyond your own

Repeatable struct attribute, appends group 1 and up (beyond the struct's own group 0), in the order written:

- `#[layout("name")]` — always `GroupEntry::Global("name")` (see [Bind Groups and Layouts](./bind-groups.md#sharing-a-layout-across-pipelines)), no extra parameter.
- `#[layout(param)]` — the caller supplies the `GroupEntry` at the call site instead — any variant, including `GroupEntry::Layout(...)` for a standalone layout that was never registered in `GlobalLayoutPool`. `into_material`/`into_compute` gains one `GroupEntry`-typed parameter per `param` occurrence, named `extra_group_0`, `extra_group_1`, ... in declaration order among the `param` occurrences specifically (fixed `#[layout("name")]` entries don't consume a slot).

```rust,ignore
#[derive(MaterialParams)]
#[layout("day_night")]              // always the shared "day_night" layout
struct TerrainMaterialParams {
    #[texture(0)]
    albedo: Handle<Texture>,
}

#[derive(MaterialParams)]
#[layout(param)]                    // caller decides — could be Global or a one-off Layout
struct EnemyMaterialParams {
    #[texture(0)]
    albedo: Handle<Texture>,
}

let terrain = TerrainMaterialParams { albedo }.into_material(Material::standard(SHADER));
let enemy = EnemyMaterialParams { albedo }.into_material(Material::standard(SHADER), GroupEntry::Global("lighting"));
```

## Parameter order

On the generated method: `base`, then one `{field}_fallback` per optional-texture-kind field (ascending binding index), then one `extra_group_N` per `#[layout(param)]` (declaration order).

## `ComputeParams`

Identical shape, targeting `Compute`'s streamlined methods instead — `.into_compute(self, base: Compute, ...) -> Compute`, visibility always `COMPUTE`, `#[storage(N)]` defaults to read-write (a compute pass binding a storage buffer usually means to write it — use `.with_entry`/`.with_storage` by hand for a read-only one):

```rust,ignore
use pebble::graphics::pipeline::compute::{Compute, ComputeParams};

#[derive(ComputeParams)]
struct BlurParams {
    #[uniform(0)]
    radius: f32,
    #[texture(1)]
    src: Handle<Texture>,
}
```

Note that, unlike `.storage(name, Vec<u8>)` on `Compute` itself, a `#[storage(N)]`/`#[uniform(N)]` *field* always goes through the typed `encase` path (same as `.storage_value`/`.uniform_value`) — its type needs to implement `encase::ShaderType`, not be raw bytes.
