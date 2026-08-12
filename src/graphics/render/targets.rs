use crate::graphics::pipeline::texture_view::TextureView;

/// One color attachment for a [`Pass`]. `attachment: None` means "the
/// swapchain's own view" — pass a [`TextureView`] to render into your own
/// texture instead (e.g. for post-processing).
pub struct ColorTarget<'a> {
    pub(crate) attachment: Option<&'a TextureView>,
    pub(crate) clear: [f32; 4]
}

pub struct ColorTargetBuilder<'a> {
    target: ColorTarget<'a>
}

impl<'a> ColorTargetBuilder<'a> {
    pub fn new() -> Self {
        Self {
            target: ColorTarget {
                attachment: None,
                clear: [0.0, 0.0, 0.0, 1.0]
            }
        }
    }

    pub fn with_attachment(mut self, view: &'a TextureView) -> Self {
        self.target.attachment = Some(view);
        self
    }

    pub fn with_clear(mut self, clear: [f32; 4]) -> Self {
        self.target.clear = clear;
        self
    }

    pub fn build(self) -> ColorTarget<'a> {
        self.target
    }
}

/// A depth attachment for a [`Pass`] — unlike a color target, always
/// pointed at your own texture (there's no swapchain depth buffer).
pub struct DepthTarget<'a> {
    pub(crate) attachment: &'a TextureView,
    pub(crate) clear: Option<f32>,
}

pub struct DepthTargetBuilder<'a> {
    target: DepthTarget<'a>
}

impl<'a> DepthTargetBuilder<'a> {
    pub fn new(view: &'a TextureView) -> Self {
        Self {
            target: DepthTarget {
                attachment: view,
                clear: Some(1.0)
            }
        }
    }

    pub fn with_clear(mut self, clear: f32) -> Self {
        self.target.clear = Some(clear);
        self
    }

    pub fn build(self) -> DepthTarget<'a> {
        self.target
    }
}

/// What to render into, passed to [`Frame::begin`](crate::graphics::render::frame::Frame::begin).
/// Zero color targets plus a depth target is a valid, depth-only pass (e.g.
/// a shadow map).
pub struct Pass<'a> {
    pub(crate) colors: Vec<ColorTarget<'a>>,
    pub(crate) depth: Option<DepthTarget<'a>>
}

pub struct PassBuilder<'a> {
    targets: Vec<ColorTarget<'a>>,
    depth: Option<DepthTarget<'a>>,
}

impl<'a> PassBuilder<'a> {
    pub fn new() -> Self {
        Self { targets: Vec::new(), depth: None }
    }

    pub fn with_target(mut self, target: ColorTarget<'a>) -> Self {
        self.targets.push(target);
        self
    }

    pub fn with_depth(mut self, depth: DepthTarget<'a>) -> Self {
        self.depth = Some(depth);
        self
    }

    pub fn build(self) -> Pass<'a> {
        Pass {
            colors: self.targets,
            depth: self.depth,
        }
    }
}
