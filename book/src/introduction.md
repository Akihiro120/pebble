# Introduction

Pebble is a low-level Rust game engine: an ECS, a wgpu-backed renderer, and an asset pipeline, wired together by a builder-style `App`. It deliberately stops there — no scene graph, no physics, no built-in skeletal animation. You get GPU primitives (buffers, textures, materials, compute pipelines) and an ECS to drive them, and you build whatever higher-level systems your game needs on top.

This book is organized by feature, not by tutorial chapter — jump to whatever you're trying to do. Each page assumes you've read [Apps and Plugins](./apps-and-plugins.md) and [Systems and Stages](./systems-and-stages.md) first, since almost everything else builds on those two.

For the full API reference, generate rustdoc locally:

```sh
cargo doc --open
```
