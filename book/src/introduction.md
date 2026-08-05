# Introduction

This book is a how-to guide to [Pebble](https://github.com/Akihiro120/pebble), a modular ECS framework for building render engines in Rust. Each page covers one feature, self-contained — "how do I add a system," "how do I render into a texture," "how do I turn on MSAA" — rather than building up one project chapter by chapter. Jump straight to the page you need; every code sample is complete enough to make sense on its own, with links to the pages it depends on where that matters.

Pebble is not a renderer. It's the plumbing *around* one: an application loop, a plugin system, a resource/entity store (built on [`hecs`](https://github.com/Ralith/hecs)), and a GPU asset pipeline that turns CPU-side descriptions into GPU-side objects on their own schedule. It ships a ready-made `wgpu` backend (`pebble::wgpu`) so you don't have to write one yourself to get started, but nothing about the framework requires it — the same `App`, systems, and asset pipeline work with a hand-rolled `Backend` implementation for Metal, Vulkan, or anything else (see [Windows and Backends](./windows-and-backends.md#owning-the-graphics-backend-yourself)).

This book uses the built-in `pebble::wgpu` module throughout, because it's the fastest path to something on screen and doesn't require understanding wgpu pipeline internals up front.

## How this book is organized

- **ECS Core** — the parts of Pebble that have nothing to do with graphics: apps and plugins, systems and stages, resources, queries and commands, events, async tasks. If you already know an ECS framework (Bevy, `hecs` directly, `specs`), skim these for Pebble's specific vocabulary (`Res`/`ResMut`, `SystemStage`, `.once()`, `run_if`).
- **Rendering: Getting Set Up** — the asset pipeline (how CPU data becomes GPU objects) and opening a window.
- **Rendering: Building Blocks** — buffers, bind groups, materials, meshes, textures, samplers: the pieces you assemble a scene from.
- **Rendering: Drawing** — recording a render pass, MSAA, render bundles, compute pipelines, and building your own one-off GPU resources (a camera, say).
- **Shipping** — running on the web, and where to look next.

If you want to see it all fit together as one running program instead of individual pages, the [`wgpu_showcase`](https://github.com/Akihiro120/pebble/tree/main/examples/wgpu_showcase) example does exactly what the "Rendering" pages describe, and [`ecs_basics`](https://github.com/Akihiro120/pebble/tree/main/examples/ecs_basics) does the same for the ECS Core pages with no window at all.

Every code sample in this book uses real, current Pebble APIs — checked against the crate this book ships alongside. Where a sample is illustrative rather than copy-pasteable (mainly in [Custom GPU Resources](./custom-gpu-resources.md)), the text says so explicitly and points at the closest full working example in `examples/`.

## Prerequisites

- Comfortable reading Rust — generics, traits, closures. Pebble leans on all three.
- No prior wgpu or graphics-API experience assumed, though some familiarity helps. Concepts (bind groups, pipelines, shader stages) are introduced as they come up, not explained from first principles — for that depth, [`learn-wgpu`](https://sotrh.github.io/learn-wgpu/) is the better reference, and Pebble's `wgpu` module is a thin, opinionated layer directly on top of what it teaches.

## A note on where this book lives

This book is source-controlled at [`book/`](https://github.com/Akihiro120/pebble/tree/main/book) in the same repository as the engine, and rebuilds automatically on every push to `main`. If a page is wrong or stale relative to the code, that's a real bug — open an issue or a PR.
