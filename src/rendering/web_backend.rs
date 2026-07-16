use crate::{
    prelude::{Backend, GPUSurfaceHandle},
    rendering::sync::{InitReceiver, InitSender},
};

pub trait AsyncInit: Backend {
    fn init_async(handle: impl GPUSurfaceHandle, width: u32, height: u32, sender: InitSender<Self>);
}

pub(crate) struct PendingBackend<B: Backend> {
    pub(crate) receiver: std::sync::Mutex<InitReceiver<B>>,
}
