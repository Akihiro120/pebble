use crate::graphics::pipeline::texture_view::TextureView;

/// One color attachment for a [`Pass`]. `attachment: None` means "the
/// swapchain's own view" — pass a [`TextureView`] to render into your own
/// texture instead (e.g. for post-processing).
pub struct ColorTarget<'a> {
    pub(crate) attachment: Option<&'a TextureView>,
    /// `Some(color)` to clear to that color, `None` to load existing contents.
    pub(crate) clear: Option<[f32; 4]>
}

pub struct ColorTargetBuilder<'a> {
    target: ColorTarget<'a>
}

impl<'a> ColorTargetBuilder<'a> {
    pub fn new() -> Self {
        Self {
            target: ColorTarget {
                attachment: None,
                clear: Some([0.0, 0.0, 0.0, 1.0])
            }
        }
    }

    pub fn with_attachment(mut self, view: &'a TextureView) -> Self {
        self.target.attachment = Some(view);
        self
    }

    /// Clear the attachment to `clear` at the start of the pass.
    pub fn with_clear(mut self, clear: [f32; 4]) -> Self {
        self.target.clear = Some(clear);
        self
    }

    /// Load the attachment's existing contents instead of clearing, so a
    /// later pass can draw on top of what an earlier one left there.
    pub fn without_clear(mut self) -> Self {
        self.target.clear = None;
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

    /// Clear the depth attachment to `clear` at the start of the pass.
    pub fn with_clear(mut self, clear: f32) -> Self {
        self.target.clear = Some(clear);
        self
    }

    /// Load the depth attachment's existing contents instead of clearing —
    /// e.g. to reuse a depth pre-pass's result in a later color pass.
    pub fn without_clear(mut self) -> Self {
        self.target.clear = None;
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

#[cfg(test)]
mod tests {
    use super::*;

    // `DepthTarget` needs a real `TextureView`, so only the color builder --
    // whose attachment is optional -- is reachable without a GPU.

    #[test]
    fn a_color_target_clears_to_opaque_black_by_default() {
        let target = ColorTargetBuilder::new().build();
        assert_eq!(target.clear, Some([0.0, 0.0, 0.0, 1.0]));
    }

    #[test]
    fn with_clear_sets_the_clear_color() {
        let target = ColorTargetBuilder::new().with_clear([0.0, 0.0, 0.0, 0.0]).build();
        assert_eq!(target.clear, Some([0.0, 0.0, 0.0, 0.0]));
    }

    #[test]
    fn without_clear_loads_existing_contents() {
        let target = ColorTargetBuilder::new().without_clear().build();
        assert_eq!(target.clear, None);
    }

    #[test]
    fn the_last_clear_setting_wins() {
        assert_eq!(ColorTargetBuilder::new().with_clear([1.0; 4]).without_clear().build().clear, None);
        assert_eq!(
            ColorTargetBuilder::new().without_clear().with_clear([1.0; 4]).build().clear,
            Some([1.0; 4])
        );
    }
}
