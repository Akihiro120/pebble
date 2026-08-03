# Introduction

This book teaches [Pebble](https://github.com/Akihiro120/pebble), a modular ECS framework for building render engines in Rust, the same way [Learn Wgpu](https://sotrh.github.io/learn-wgpu/) teaches wgpu: by building something real, one small step at a time, explaining each new piece only once it's actually needed.

Pebble is not a renderer. It's the plumbing *around* one: an application loop, a plugin system, a resource/entity store (built on [`hecs`](https://github.com/Ralith/hecs)), and a GPU asset pipeline that turns CPU-side descriptions into GPU-side objects on their own schedule. It ships a ready-made `wgpu` backend (`pebble::wgpu`) so you don't have to write one yourself to get started, but nothing about the framework requires it — the same `App`, systems, and asset pipeline work with a hand-rolled `Backend` implementation for Metal, Vulkan, or anything else.

This book uses the built-in `pebble::wgpu` module throughout, because it's the fastest path to something on screen and doesn't require understanding wgpu pipeline internals up front. [Where to Go From Here](./ch13-next-steps.md) points at what changes if you outgrow it and want to own the graphics backend directly.

## What you'll build

By the end of Part II you'll have a window, a textured quad rendered through the asset pipeline, an orbiting camera with a depth buffer, and a compute pass — the same arc as `learn-wgpu`'s early chapters, adapted to how Pebble structures things: as `Plugin`s, `System`s, and `Asset`s instead of one big `run()` function.

## How to read this

- **Part I** covers the ECS core — the parts of Pebble that have nothing to do with graphics. If you already know an ECS framework (Bevy, `hecs` directly, `specs`), skim it for Pebble's specific vocabulary (`Res`/`ResMut`, `SystemStage`, `.once()`, `run_if`) and move on.
- **Part II** is the hands-on rendering tutorial, building up one example project chapter by chapter.
- **Part III** covers running the result on the web and where to go next.

Every code sample in this book uses real, current Pebble APIs — checked against the crate this book ships alongside. Where a sample is illustrative rather than copy-pasteable (mainly in [Camera, Depth, and Lazy Resources](./ch10-camera-and-depth.md) and [Compute Pipelines](./ch11-compute.md)), the text says so explicitly and points at the closest full working example in `examples/`.

## Prerequisites

- Comfortable reading Rust — generics, traits, closures. Pebble leans on all three.
- No prior wgpu or graphics-API experience assumed, though some familiarity helps. Concepts (bind groups, pipelines, shader stages) are introduced as they come up, not explained from first principles — for that depth, `learn-wgpu` itself is the better reference, and Pebble's `wgpu` module is a thin, opinionated layer directly on top of what it teaches.

## A note on where this book lives

This book is source-controlled at [`book/`](https://github.com/Akihiro120/pebble/tree/main/book) in the same repository as the engine, and rebuilds automatically on every push to `main`. If a chapter is wrong or stale relative to the code, that's a real bug — open an issue or a PR.
