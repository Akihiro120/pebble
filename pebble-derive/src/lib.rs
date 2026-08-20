//! Derive macros for `pebble-engine`'s `Material`/`Compute` — turns a plain
//! struct's fields into the same `.with_entry_at(...)`/`.texture(...)`/
//! `.uniform_value(...)`/etc. chain you'd otherwise write by hand against
//! `Material`/`Compute`, so the struct's shape *is* the bind group instead
//! of a separate declaration to keep in sync with it.
//!
//! Field attributes: `#[uniform(N)]`, `#[storage(N)]`, `#[texture(N)]`,
//! `#[texture_array(N)]`, `#[cubemap(N)]`, `#[sampler(N)]` — `N` is the real
//! WGSL `@binding(N)` index. Several `#[uniform(N)]`/`#[storage(N)]` fields
//! sharing the same `N` are packed into one generated buffer together
//! (matching how a WGSL `uniform`/`storage` block is one binding no matter
//! how many fields it has); every other kind needs a binding to itself.
//!
//! **Visibility.** Defaults to `FRAGMENT` for `MaterialParams`, always
//! exactly `COMPUTE` for `ComputeParams` (a compute bind group entry can't
//! be anything else — `#[derive(ComputeParams)]` rejects any override).
//! Override per-field on a `MaterialParams` struct with a second attribute
//! argument: `#[uniform(0, vertex)]`, `#[texture(1, vertex_fragment)]` —
//! `vertex`, `fragment`, or `vertex_fragment`. Every field sharing a
//! grouped `#[uniform(N)]`/`#[storage(N)]` index must agree on the same
//! visibility (explicit or all-default).
//!
//! **Optional textures.** A `#[texture(N)]`/`#[texture_array(N)]`/
//! `#[cubemap(N)]` field typed `Option<Handle<T>>` instead of `Handle<T>`
//! binds a fallback when the value is `None` — since the WGSL binding
//! always exists regardless of whether a given instance has a value, the
//! generated `into_material`/`into_compute` gains one extra
//! `{field}_fallback: Handle<T>` parameter per optional field, used only
//! for the `None` case.
//!
//! **Type checking.** The macro doesn't deeply understand types — it just
//! emits a call to the matching `Material`/`Compute` method. But
//! `#[texture(N)]`/`#[texture_array(N)]`/`#[cubemap(N)]`/`#[sampler(N)]`
//! fields are checked against the shape they're expected to be
//! (`Handle<Texture>`/`Option<Handle<Texture>>` for `#[texture]`, etc.) on
//! a best-effort basis — recognized and wrong (e.g. `#[texture(1)] foo:
//! Handle<Cubemap>`) is a clear compile error pointing at the field; not
//! confidently recognized (a type alias, an unusual path) silently defers
//! to rustc's own type error at the generated call site, same as before.
//!
//! Struct attribute `#[layout(...)]` (repeatable) appends a bind group
//! beyond the struct's own (group 0), in the order written:
//! - `#[layout("name")]` — always `GroupEntry::Global("name")`, no extra
//!   parameter.
//! - `#[layout(param)]` — the caller supplies the `GroupEntry` at the call
//!   site instead (any variant, including `GroupEntry::Layout(...)` for a
//!   standalone layout that was never registered in `GlobalLayoutPool`);
//!   `into_material`/`into_compute` gains one `GroupEntry`-typed parameter
//!   per `param` occurrence, named `extra_group_0`, `extra_group_1`, ... in
//!   declaration order among the `param` occurrences specifically (fixed
//!   `#[layout("name")]` entries don't consume a slot).
//!
//! Parameter order on the generated method: `base`, then one
//! `{field}_fallback` per optional-texture-kind field (ascending binding
//! index), then one `extra_group_N` per `#[layout(param)]` (declaration
//! order).

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use std::collections::BTreeMap;
use syn::{Data, DeriveInput, Field, Fields, GenericArgument, Ident, LitInt, LitStr, PathArguments, Type, parse_macro_input};

#[proc_macro_derive(
    MaterialParams,
    attributes(uniform, storage, texture, texture_array, cubemap, sampler, layout)
)]
pub fn derive_material_params(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand(input, Mode::Material).into()
}

#[proc_macro_derive(
    ComputeParams,
    attributes(uniform, storage, texture, texture_array, cubemap, sampler, layout)
)]
pub fn derive_compute_params(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand(input, Mode::Compute).into()
}

// `encase`'s own `ShaderType` derive hardcodes `::encase::...` paths in the
// code it generates, so it only works in crates that depend on `encase`
// directly. Building our own copy of that derive macro — pointed at
// `::pebble::encase` (re-exported from pebble's crate root) instead — lets
// `#[derive(MaterialParams)]`'s generated group structs use it without
// requiring downstream consumers to add `encase` as a dependency of their
// own. Re-exported as `pebble::EncaseShaderType`.
encase_derive_impl::implement!(encase_derive_impl::syn::parse_quote!(::pebble::encase));

#[derive(Clone, Copy)]
enum Mode {
    Material,
    Compute,
}

impl Mode {
    fn base_ty(self) -> TokenStream2 {
        match self {
            Mode::Material => quote!(::pebble::graphics::pipeline::material::Material),
            Mode::Compute => quote!(::pebble::graphics::pipeline::compute::Compute),
        }
    }

    fn method_name(self) -> Ident {
        match self {
            Mode::Material => format_ident!("into_material"),
            Mode::Compute => format_ident!("into_compute"),
        }
    }

    fn default_visibility(self) -> TokenStream2 {
        match self {
            Mode::Material => quote!(::pebble::graphics::types::flags::ShaderStages::FRAGMENT),
            Mode::Compute => quote!(::pebble::graphics::types::flags::ShaderStages::COMPUTE),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Uniform,
    Storage,
    Texture,
    TextureArray,
    Cubemap,
    Sampler,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::Uniform => "uniform",
            Kind::Storage => "storage",
            Kind::Texture => "texture",
            Kind::TextureArray => "texture_array",
            Kind::Cubemap => "cubemap",
            Kind::Sampler => "sampler",
        }
    }

    fn from_ident(name: &str) -> Option<Self> {
        match name {
            "uniform" => Some(Kind::Uniform),
            "storage" => Some(Kind::Storage),
            "texture" => Some(Kind::Texture),
            "texture_array" => Some(Kind::TextureArray),
            "cubemap" => Some(Kind::Cubemap),
            "sampler" => Some(Kind::Sampler),
            _ => None,
        }
    }

    /// The `Handle<...>` inner type name this kind expects, for the
    /// best-effort type check. `None` for kinds that aren't `Handle<T>`-shaped.
    fn expected_handle_name(self) -> Option<&'static str> {
        match self {
            Kind::Texture => Some("Texture"),
            Kind::TextureArray => Some("TextureArray"),
            Kind::Cubemap => Some("Cubemap"),
            Kind::Uniform | Kind::Storage | Kind::Sampler => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VisOverride {
    Vertex,
    Fragment,
    VertexFragment,
    Compute,
}

impl VisOverride {
    fn from_ident(name: &str) -> Option<Self> {
        match name {
            "vertex" => Some(VisOverride::Vertex),
            "fragment" => Some(VisOverride::Fragment),
            "vertex_fragment" => Some(VisOverride::VertexFragment),
            "compute" => Some(VisOverride::Compute),
            _ => None,
        }
    }

    fn tokens(self) -> TokenStream2 {
        let path = quote!(::pebble::graphics::types::flags::ShaderStages);
        match self {
            VisOverride::Vertex => quote!(#path::VERTEX),
            VisOverride::Fragment => quote!(#path::FRAGMENT),
            VisOverride::VertexFragment => quote!(#path::VERTEX_FRAGMENT),
            VisOverride::Compute => quote!(#path::COMPUTE),
        }
    }
}

/// If `ty` is `Handle<X>` (matched by the last path segment's name, not the
/// full path — the practical heuristic, since almost everyone just writes
/// `Handle<Texture>` regardless of how they imported `Handle`), returns
/// `X`. `None` for anything else, including `Option<Handle<X>>` — see
/// [`as_option_handle`].
fn as_handle(ty: &Type) -> Option<&Type> {
    let Type::Path(p) = ty else { return None };
    let seg = p.path.segments.last()?;
    if seg.ident != "Handle" {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &seg.arguments else { return None };
    args.args.iter().find_map(|a| match a {
        GenericArgument::Type(t) => Some(t),
        _ => None,
    })
}

/// If `ty` is `Option<Handle<X>>`, returns `X`.
fn as_option_handle(ty: &Type) -> Option<&Type> {
    let Type::Path(p) = ty else { return None };
    let seg = p.path.segments.last()?;
    if seg.ident != "Option" {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &seg.arguments else { return None };
    let inner = args.args.iter().find_map(|a| match a {
        GenericArgument::Type(t) => Some(t),
        _ => None,
    })?;
    as_handle(inner)
}

/// The last path segment's name, e.g. `"Texture"` for `Texture` or
/// `crate::assets::Texture`. Used only for the best-effort type check.
fn last_segment_name(ty: &Type) -> Option<String> {
    let Type::Path(p) = ty else { return None };
    Some(p.path.segments.last()?.ident.to_string())
}

struct FieldSpec {
    ident: Ident,
    kind: Kind,
    index: u32,
    visibility: Option<(VisOverride, Span)>,
    /// `Some(X)` if this field's type is `Option<Handle<X>>` — only
    /// meaningful for `Texture`/`TextureArray`/`Cubemap` kinds.
    optional_inner: Option<Type>,
}

fn parse_binding_attr(attr: &syn::Attribute) -> syn::Result<(u32, Option<(VisOverride, Span)>)> {
    attr.parse_args_with(|input: syn::parse::ParseStream| {
        let index: LitInt = input.parse()?;
        let index = index.base10_parse()?;
        if input.is_empty() {
            return Ok((index, None));
        }
        input.parse::<syn::Token![,]>()?;
        let ident: Ident = input.parse()?;
        let Some(vis) = VisOverride::from_ident(&ident.to_string()) else {
            return Err(syn::Error::new_spanned(
                &ident,
                format!("unknown visibility `{ident}` — expected `vertex`, `fragment`, `vertex_fragment`, or `compute`"),
            ));
        };
        Ok((index, Some((vis, ident.span()))))
    })
}

/// The one recognized binding attribute on `field`, including a best-effort
/// check that its type matches what that attribute expects.
fn field_spec(field: &Field) -> syn::Result<FieldSpec> {
    let mut found: Option<(Kind, u32, Option<(VisOverride, Span)>)> = None;
    for attr in &field.attrs {
        let Some(kind) = attr.path().get_ident().and_then(|i| Kind::from_ident(&i.to_string())) else {
            continue;
        };
        if found.is_some() {
            return Err(syn::Error::new_spanned(
                attr,
                "a field can only have one of #[uniform]/#[storage]/#[texture]/#[texture_array]/#[cubemap]/#[sampler]",
            ));
        }
        let (index, visibility) = parse_binding_attr(attr)?;
        found = Some((kind, index, visibility));
    }
    let Some((kind, index, visibility)) = found else {
        return Err(syn::Error::new_spanned(
            field,
            "every field needs one of #[uniform(N)]/#[storage(N)]/#[texture(N)]/#[texture_array(N)]/#[cubemap(N)]/#[sampler(N)]",
        ));
    };
    let ident = field.ident.clone().expect("named field");

    let mut optional_inner = None;
    if let Some(expected) = kind.expected_handle_name() {
        // best-effort: only ever error when we're confident (the shape
        // recognizably resolves to a `Handle<X>`/`Option<Handle<X>>` whose
        // `X` doesn't match) — anything we don't recognize (a type alias,
        // an unusual path) is silently left for rustc's own type error at
        // the generated call site, same as if this check didn't exist.
        let (inner, is_optional) = match as_option_handle(&field.ty) {
            Some(inner) => (Some(inner), true),
            None => (as_handle(&field.ty), false),
        };
        if let Some(inner_ty) = inner {
            if let Some(name) = last_segment_name(inner_ty) {
                if name != expected {
                    let shape = if is_optional { format!("Option<Handle<{expected}>>") } else { format!("Handle<{expected}>") };
                    return Err(syn::Error::new_spanned(
                        &field.ty,
                        format!(
                            "field `{ident}` is #[{}({index})] so its type should be `{shape}` — found `Handle<{name}>`",
                            kind.label()
                        ),
                    ));
                }
            }
            if is_optional {
                optional_inner = Some(inner_ty.clone());
            }
        }
    } else if kind == Kind::Sampler {
        // no Option support for samplers (not asked for), but still catch
        // the common slip of reaching for #[sampler] on a texture field.
        if as_handle(&field.ty).is_some() || as_option_handle(&field.ty).is_some() {
            return Err(syn::Error::new_spanned(
                &field.ty,
                format!("field `{ident}` is #[sampler({index})] but its type looks like a `Handle<...>` — did you mean #[texture({index})]?"),
            ));
        }
    }

    Ok(FieldSpec { ident, kind, index, visibility, optional_inner })
}

enum LayoutSpec {
    Global(LitStr),
    Param,
}

fn layout_specs(attrs: &[syn::Attribute]) -> syn::Result<Vec<LayoutSpec>> {
    attrs
        .iter()
        .filter(|a| a.path().is_ident("layout"))
        .map(|attr| {
            attr.parse_args_with(|input: syn::parse::ParseStream| {
                if let Ok(lit) = input.parse::<LitStr>() {
                    Ok(LayoutSpec::Global(lit))
                } else {
                    let ident: Ident = input.parse()?;
                    if ident == "param" {
                        Ok(LayoutSpec::Param)
                    } else {
                        Err(syn::Error::new_spanned(
                            ident,
                            "expected a string literal (a GlobalLayoutPool name) or the keyword `param`",
                        ))
                    }
                }
            })
        })
        .collect()
}

fn expand(input: DeriveInput, mode: Mode) -> TokenStream2 {
    let struct_name = &input.ident;

    let Data::Struct(data) = &input.data else {
        return syn::Error::new_spanned(&input, "MaterialParams/ComputeParams can only be derived for a struct")
            .to_compile_error();
    };
    let Fields::Named(fields) = &data.fields else {
        return syn::Error::new_spanned(&input, "MaterialParams/ComputeParams requires named fields").to_compile_error();
    };

    let layouts = match layout_specs(&input.attrs) {
        Ok(v) => v,
        Err(e) => return e.to_compile_error(),
    };

    let mut specs = Vec::with_capacity(fields.named.len());
    for field in &fields.named {
        match field_spec(field) {
            Ok(spec) => specs.push((spec, field.ty.clone())),
            Err(e) => return e.to_compile_error(),
        }
    }

    // Compute mode has exactly one valid visibility — reject overrides
    // outright, up front, rather than letting a mismatched-group error
    // (below) obscure the real problem.
    if matches!(mode, Mode::Compute) {
        for (spec, _) in &specs {
            if let Some((_, span)) = spec.visibility {
                return syn::Error::new(
                    span,
                    "visibility overrides aren't valid on #[derive(ComputeParams)] — every compute bind group entry must be exactly COMPUTE-visible",
                )
                .to_compile_error();
            }
        }
    } else {
        for (spec, _) in &specs {
            if let Some((VisOverride::Compute, span)) = spec.visibility {
                return syn::Error::new(span, "`compute` visibility isn't valid on #[derive(MaterialParams)] — a material bind group entry can't be COMPUTE-visible")
                    .to_compile_error();
            }
        }
    }

    let mut groups: BTreeMap<u32, Vec<(FieldSpec, Type)>> = BTreeMap::new();
    for (spec, ty) in specs {
        groups.entry(spec.index).or_default().push((spec, ty));
    }

    let base_ty = mode.base_ty();
    let method_name = mode.method_name();
    let default_visibility = mode.default_visibility();
    let binding_kind = quote!(::pebble::graphics::pipeline::binding::BindingKind);
    let handle_ty = quote!(::pebble::assets::handle::Handle);

    let mut chain_calls = Vec::new();
    let mut group_struct_defs = Vec::new();
    let mut fallback_params = Vec::new();

    for (index, entries) in &groups {
        let kind = entries[0].0.kind;
        for (spec, _) in entries.iter().skip(1) {
            if spec.kind != kind {
                let msg = format!(
                    "binding {index} is used by both #[{}] and #[{}] — a wgpu binding slot can only be one kind",
                    kind.label(),
                    spec.kind.label()
                );
                return syn::Error::new_spanned(&spec.ident, msg).to_compile_error();
            }
        }
        let visibility = entries[0].0.visibility.map(|(v, _)| v);
        for (spec, _) in entries.iter().skip(1) {
            let this_vis = spec.visibility.map(|(v, _)| v);
            if this_vis != visibility {
                return syn::Error::new_spanned(
                    &spec.ident,
                    format!("every field sharing binding {index} must specify the same visibility (or all leave it at the default)"),
                )
                .to_compile_error();
            }
        }
        let visibility = visibility.map(VisOverride::tokens).unwrap_or_else(|| default_visibility.clone());

        let name = entries[0].0.ident.to_string();
        let name_lit = LitStr::new(&name, entries[0].0.ident.span());

        match kind {
            Kind::Uniform | Kind::Storage => {
                let group_ident = format_ident!("__{struct_name}Group{index}");
                let field_defs = entries.iter().map(|(spec, ty)| {
                    let ident = &spec.ident;
                    quote!(#ident: #ty)
                });
                let field_inits = entries.iter().map(|(spec, _)| {
                    let ident = &spec.ident;
                    quote!(#ident: self.#ident)
                });

                group_struct_defs.push(quote! {
                    #[derive(::pebble::EncaseShaderType)]
                    struct #group_ident { #(#field_defs),* }
                });

                let (kind_ctor, method) = if kind == Kind::Uniform {
                    (quote!(#binding_kind::uniform_buffer(#visibility)), format_ident!("with_uniform_value"))
                } else {
                    (quote!(#binding_kind::storage_buffer_read_only(#visibility)), format_ident!("with_storage_value"))
                };

                chain_calls.push(quote! {
                    __material = __material
                        .with_entry_at(#name_lit, #index, #kind_ctor)
                        .#method(#name_lit, &#group_ident { #(#field_inits),* });
                });
            }
            Kind::Texture | Kind::TextureArray | Kind::Cubemap | Kind::Sampler => {
                if let Some((extra, _)) = entries.get(1) {
                    let msg = format!(
                        "binding {index} is used by more than one #[{}] field — only #[uniform]/#[storage] fields can share a binding",
                        kind.label()
                    );
                    return syn::Error::new_spanned(&extra.ident, msg).to_compile_error();
                }
                let (spec, _) = &entries[0];
                let ident = &spec.ident;
                let (kind_ctor, method) = match kind {
                    Kind::Texture => (quote!(#binding_kind::texture_2d(#visibility)), format_ident!("with_texture")),
                    Kind::TextureArray => {
                        (quote!(#binding_kind::texture_2d_array(#visibility)), format_ident!("with_texture_array"))
                    }
                    Kind::Cubemap => (quote!(#binding_kind::texture_cubemap(#visibility)), format_ident!("with_cubemap")),
                    Kind::Sampler => (quote!(#binding_kind::sampler(#visibility)), format_ident!("with_sampler")),
                    Kind::Uniform | Kind::Storage => unreachable!(),
                };

                let value = if let Some(inner) = &spec.optional_inner {
                    let fallback_ident = format_ident!("{ident}_fallback");
                    fallback_params.push(quote!(#fallback_ident: #handle_ty<#inner>));
                    quote!(self.#ident.unwrap_or(#fallback_ident))
                } else {
                    quote!(self.#ident)
                };

                chain_calls.push(quote! {
                    __material = __material
                        .with_entry_at(#name_lit, #index, #kind_ctor)
                        .#method(#name_lit, #value);
                });
            }
        }
    }

    let group_entry_ty = quote!(::pebble::graphics::pipeline::layout::GroupEntry);
    let mut extra_params = Vec::new();
    let mut extra_calls = Vec::new();
    let mut param_index = 0u32;
    for layout in &layouts {
        match layout {
            LayoutSpec::Global(name) => {
                extra_calls.push(quote! {
                    __material = __material.with_extra_group(#group_entry_ty::Global(#name));
                });
            }
            LayoutSpec::Param => {
                let param_ident = format_ident!("extra_group_{param_index}");
                param_index += 1;
                extra_params.push(quote!(#param_ident: #group_entry_ty));
                extra_calls.push(quote! {
                    __material = __material.with_extra_group(#param_ident);
                });
            }
        }
    }

    quote! {
        #(#group_struct_defs)*

        impl #struct_name {
            pub fn #method_name(self, base: #base_ty, #(#fallback_params,)* #(#extra_params),*) -> #base_ty {
                let mut __material = base;
                #(#chain_calls)*
                #(#extra_calls)*
                __material
            }
        }
    }
}
