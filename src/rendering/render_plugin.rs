use crate::{Backend, CurrentFrame, Plugin, ResMut, SystemStage};

pub struct RenderPlugin<B: Backend> {
    _marker: std::marker::PhantomData<B>,
}

impl<B: Backend> RenderPlugin<B> {
    pub fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

impl<B: Backend> Plugin for RenderPlugin<B> {
    fn build(&self, app: &mut crate::prelude::App) {
        app.add_resource(CurrentFrame::<B> { frame: None })
            .add_system(SystemStage::PreRender, begin_frame::<B>)
            .add_system(SystemStage::PostRender, end_frame::<B>);
    }
}

fn begin_frame<B: Backend>(backend: Option<ResMut<B>>, mut frame: ResMut<CurrentFrame<B>>) {
    let Some(mut backend) = backend else { return };
    frame.frame = backend.acquire();
}

fn end_frame<B: Backend>(backend: Option<ResMut<B>>, mut current: ResMut<CurrentFrame<B>>) {
    let Some(mut backend) = backend else { return };
    if let Some(frame) = current.frame.take() {
        backend.present(frame);
    }
}
