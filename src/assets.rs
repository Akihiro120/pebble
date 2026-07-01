use crate::Plugin;

pub trait AssetProvider {
    type GPU;
    type GPUOutput;

    fn upload();
}

pub struct AssetPlugin {}

impl Plugin for AssetPlugin {
    fn build(&self, app: &mut crate::prelude::App) {}
}

// implement for Mesh

// implement for Material

// implement for Audio

// implement for Textures
