use std::collections::HashMap;

use crate::Plugin;

pub trait AssetLoader: 'static {
    type GPUCtx: 'static + Send + Sync;
    type GPUOutput: 'static;
    type Output: 'static;

    // fn load<T>(&self) -> Self::Output;
    fn sync(&self, cpu: Self::Output, gpu: Self::GPUCtx) -> Self::GPUOutput;
}

pub struct AssetPlugin<L: AssetLoader> {
    loader: L,
}

impl<L> AssetPlugin<L>
where
    L: AssetLoader + 'static,
{
    pub fn new<F>(loader: L, func: F) -> Self
    where
        F: FnOnce(&L),
    {
        func(&loader);

        Self { loader }
    }
}

impl<L> Plugin for AssetPlugin<L>
where
    L: AssetLoader + 'static,
{
    fn build(&self, app: &mut crate::prelude::App) {}
}

pub struct Handle<T: 'static> {
    id: u32,
    _marker: std::marker::PhantomData<T>,
}

pub struct Asset<T: 'static> {
    storage: Vec<T>,
    names: HashMap<String, u32>,
}

impl<T> Asset<T>
where
    T: 'static,
{
    pub fn new() -> Self {
        Self {
            storage: Vec::new(),
            names: HashMap::new(),
        }
    }
}

struct AssetQueue<L> {
    loader: L,
}

impl<L> AssetQueue<L>
where
    L: AssetLoader + 'static,
{
    pub fn new(loader: L) -> Self {
        Self { loader }
    }
}

// implement for Mesh

// implement for Material

// implement for Audio

// implement for Textures
