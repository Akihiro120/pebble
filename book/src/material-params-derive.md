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

## Full example: the struct and its shader, side by side

The struct above declares three bindings in group 0: `tint`+`emissive` packed into one uniform buffer at binding 0 (same index, so one WGSL `var<uniform>`), `albedo` at binding 1, `sampler` at binding 2. The WGSL has to declare the *same* shape — the derive doesn't generate your shader, only the Rust-side wiring:

```wgsl
// enemy.wgsl
struct Params {
    tint: vec4<f32>,
    emissive: f32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var albedo: texture_2d<f32>;
@group(0) @binding(2) var albedo_sampler: sampler;

// matches Vertex::layout() — position/tex_coords/normal/tangent,
// what Material::standard() wires up for you
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) tangent: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(in.position, 1.0);
    out.tex_coords = in.tex_coords;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let base = textureSample(albedo, albedo_sampler, in.tex_coords);
    return base * params.tint + vec4<f32>(vec3<f32>(params.emissive), 0.0);
}
```

```rust,ignore
use pebble::graphics::pipeline::{material::{Material, MaterialParams}, samplers::SamplerKind, textures::Texture};

const ENEMY_SHADER: &str = include_str!("enemy.wgsl");

#[derive(MaterialParams)]
struct EnemyMaterialParams {
    #[uniform(0)]
    tint: Vec4,
    #[uniform(0)]
    emissive: f32,
    #[texture(1)]
    albedo: Handle<Texture>,
    #[sampler(2)]
    sampler: SamplerKind,
}

fn setup(mut materials: Write<Assets<Material>>, mut textures: Write<Assets<Texture>>) {
    let albedo = Texture::from_file("enemy.png").build_asset("enemy_albedo", &mut textures);

    let red = EnemyMaterialParams {
        tint: Vec4::new(1.0, 0.3, 0.3, 1.0),
        emissive: 0.0,
        albedo,
        sampler: SamplerKind::LinearRepeat,
    }
    .into_material(Material::standard(ENEMY_SHADER))   // Vertex::layout() + Depth32Float + real surface format
    .build_asset("enemy_red", &mut materials);
}
```

`Material::standard(ENEMY_SHADER)` is what supplies the vertex layout the shader's `VertexInput` assumes (see [Materials](./materials.md#materialstandard-presets-for-the-common-case)) — the derive only ever touches the bind-group side (group 0 here), never the vertex/fixed-function state, so `.standard(...)`/`.new(...)` and `.with_vertex_layouts(...)`/`.with_depth(...)`/etc. still work exactly as described there.

## Field attributes

`#[uniform(N)]`, `#[storage(N)]`, `#[texture(N)]`, `#[texture_array(N)]`, `#[cubemap(N)]`, `#[sampler(N)]` — every field needs exactly one. `N` is the real WGSL `@binding(N)` index, same as `.with_entry_at`'s.

**Grouping.** Several `#[uniform(N)]`/`#[storage(N)]` fields sharing the same `N` pack into one generated buffer together (named after the first field in the group) — matching how a WGSL `uniform`/`storage` block is one binding no matter how many members it has. Every other kind needs a binding to itself; two fields sharing a `#[texture(N)]`/etc. index is a compile error.

**Type checking.** `#[texture(N)]`/`#[texture_array(N)]`/`#[cubemap(N)]`/`#[sampler(N)]` fields are checked against the shape they're expected to be, on a best-effort basis: a field type that's recognizably wrong (`#[texture(1)] foo: Handle<Cubemap>`) is a clear compile error pointing at the field; anything not confidently recognized (a type alias, an unusual path) is silently left to rustc's own type error at the generated call site, same as if the check didn't exist. `#[uniform]`/`#[storage]` fields can be any type implementing `encase::ShaderType` (see [Materials](./materials.md#typed-uniforms-with-encase)) — there's no fixed shape to check there.

## Combining with manual bindings

Not every value type has an attribute — a comparison sampler, a pre-built texture view/storage texture, or an existing `Buffer`/`DynamicBuffer` you bind directly have no `#[...]` form (see the [full value type table](./materials.md#every-value-type-and-how-to-add-it)). For those, add them by hand alongside the derived ones: `.into_material(...)`/`.into_compute(...)` return an ordinary `Material`/`Compute`, so `.with_entry(...)` + a value call just chain onto the result, same as if the derive weren't there at all:

```rust,ignore
#[derive(MaterialParams)]
struct EnemyMaterialParams {
    #[texture(0)]
    albedo: Handle<Texture>,
    #[sampler(1)]
    albedo_sampler: SamplerKind,
}

let mat = EnemyMaterialParams { albedo, albedo_sampler: SamplerKind::LinearRepeat }
    .into_material(Material::standard(SHADER))   // claims bindings 0 and 1
    // manual entries appended after — auto-assigned indices pick up at 2
    .with_entry("shadow_map", BindingKind::texture_2d(ShaderStages::FRAGMENT))
    .with_texture_view("shadow_map", shadow_view)
    .with_entry("shadow_sampler", BindingKind::comparison_sampler(ShaderStages::FRAGMENT))
    .with_sampler("shadow_sampler", SamplerKind::CompareLess)
    .build_asset("enemy", &mut materials);
```

**Order matters for auto-assigned indices.** `#[uniform(N)]`/etc. fields always claim their literal `N` — same as `.with_entry_at` — regardless of where `.into_material(...)` sits in the chain. But a manual `.with_entry(name, kind)` (or a streamlined call like `.texture(...)`) auto-assigns the *next* free index, tracked on the same `Material`/`Compute` you're building. Chain manual auto-indexed entries **after** `.into_material(...)`/`.into_compute(...)` so they pick up after the struct's own indices; chaining them onto `base` *before* passing it in risks colliding with a low `N` the struct declares (a fresh `Material::standard(...)` starts auto-assignment at 0, same as a struct's first `#[texture(0)]`). If you do want manual entries declared first, pin them with `.with_entry_at(name, N, kind)` at an index above the struct's highest one instead of relying on auto-assignment.

**Collisions aren't silent.** Two entries (derived or manual) landing on the same binding index panics with a clear message (`binding N assigned more than once building bind group layout...`) the first time the material/compute actually builds its pipeline — not at derive-macro compile time, since the derive has no visibility into what a caller chains on afterward. Names need to stay unique the same way — a derived field and a manual entry sharing a name silently resolve to whichever one was declared first (see [Materials](./materials.md#bind-group-values-streamlined-vs-manual) on how names and values match up), rather than erroring.

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
