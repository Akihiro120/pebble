//! Shared helper for `wgpu`-module tests that need a real `wgpu::Device` —
//! pipeline/bind-group-layout construction can't be faked, since
//! `wgpu::Device`/`BindGroupLayout`/etc. are opaque handles with no public
//! constructor of their own.
//!
//! CI (and some dev machines) may have no usable GPU adapter at all — rather
//! than hard-failing the whole suite on a runner with no graphics support,
//! [`try_device`] returns `None` and callers skip with a message, the same
//! pattern `wgpu`'s own test suite uses. This means these tests only
//! *actually* run wherever an adapter is available, but they never turn a
//! missing GPU into a red CI build, and still give real coverage locally
//! and on any runner that does have one (including a software one).

pub(crate) fn try_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    pollster::block_on(async {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            display: None,
            backends: wgpu::Backends::PRIMARY,
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::default(),
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::None,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .ok()?;

        adapter.request_device(&wgpu::DeviceDescriptor::default()).await.ok()
    })
}

/// Run `test` with a real device, or skip (printing why) if none is
/// available on this machine. Call at the top of any `#[test]` that needs
/// `wgpu::Device`/`wgpu::Queue`.
macro_rules! with_device {
    ($device:ident, $queue:ident, $body:block) => {
        match crate::wgpu::test_util::try_device() {
            Some(($device, $queue)) => $body,
            None => {
                eprintln!("skipping: no wgpu adapter available on this machine");
            }
        }
    };
}
pub(crate) use with_device;
