//! GPU timestamp queries — real per-pass GPU time, independent of vsync.
//!
//! Per frame: [`begin_frame`](GpuTimestamps::begin_frame), then the `_timed`
//! pass constructors on [`Frame`](super::frame::Frame), then
//! [`Frame::resolve_timestamps`](super::frame::Frame::resolve_timestamps).
//! Read the destination back on a LATER tick with
//! [`Buffer::read`](crate::graphics::pipeline::buffers::Buffer::read) and pass
//! the bytes to [`decode`](GpuTimestamps::decode): the resolve rides the
//! frame's own command buffers, so a same-tick read submits ahead of it and
//! returns the previous contents.

use crate::graphics::{render::Backend, types::flags::DeviceFeatures};

/// A pool of timestamp query pairs, one pair per timed pass.
pub struct GpuTimestamps {
    set: wgpu::QuerySet,
    capacity: u32,
    used: u32,
    labels: Vec<&'static str>,
    /// Labels as of the last resolve. The readback is a frame behind, so
    /// decoding against the live list mislabels every timing on a frame that
    /// recorded a different set of passes.
    resolved: Vec<&'static str>,
    period_ns: f32,
}

impl GpuTimestamps {
    /// Allocates `capacity` scopes. `None` when the device wasn't granted
    /// `TIMESTAMP_QUERY`, which callers are expected to treat as "carry on
    /// unprofiled" rather than as an error.
    pub fn new(backend: &Backend, capacity: u32) -> Option<Self> {
        if !backend.features().contains(DeviceFeatures::TIMESTAMP_QUERY) {
            return None;
        }

        let set = backend.device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("pebble timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: capacity * 2,
        });

        Some(Self {
            set,
            capacity,
            used: 0,
            labels: Vec::with_capacity(capacity as usize),
            resolved: Vec::with_capacity(capacity as usize),
            period_ns: backend.queue.get_timestamp_period(),
        })
    }

    /// Byte size the resolve destination buffer must have.
    pub fn resolve_size(&self) -> u64 {
        self.capacity as u64 * 2 * size_of::<u64>() as u64
    }

    /// Clears last frame's scopes. Call once, before the first timed pass.
    pub fn begin_frame(&mut self) {
        self.used = 0;
        self.labels.clear();
    }

    /// Claims a scope, returning its (begin, end) query indices. `None` once
    /// `capacity` is exhausted — silently dropping a scope beats panicking
    /// mid-frame over a diagnostic.
    pub(crate) fn claim(&mut self, label: &'static str) -> Option<(u32, u32)> {
        if self.used >= self.capacity {
            return None;
        }
        let begin = self.used * 2;
        self.used += 1;
        self.labels.push(label);
        Some((begin, begin + 1))
    }

    pub(crate) fn raw(&self) -> &wgpu::QuerySet {
        &self.set
    }

    pub(crate) fn used(&self) -> u32 {
        self.used
    }

    pub(crate) fn snapshot_resolved(&mut self) {
        self.resolved.clone_from(&self.labels);
    }

    /// Resolved bytes to `(label, begin_us, end_us)`, in recording order, with
    /// both ends relative to the earliest timestamp in the frame.
    ///
    /// [`decode`](Self::decode) reports `end - begin` per scope, which only
    /// means "what this pass cost" if scopes don't overlap. Render passes
    /// routinely do: a tile-based GPU processes a frame's render passes as one
    /// batch, so every pass's pair brackets the whole batch, each reports the
    /// batch's duration, and summing them multiplies it. Measured on an
    /// M-series Mac, three render passes reported begins within 0.6M ticks of
    /// each other and ends within 0.03M — they are one interval, not three.
    ///
    /// Intervals let the caller merge overlapping scopes and take the union.
    /// A scope whose end never advanced — some backends leave the last pass's
    /// query unwritten — comes back with `end == begin` rather than being
    /// dropped, so it can be reported as unmeasured instead of vanishing.
    pub fn decode_intervals(&self, bytes: &[u8]) -> Vec<(&'static str, f64, f64)> {
        let ticks: Vec<u64> = bytes
            .chunks_exact(8)
            .map(|c| u64::from_le_bytes(c.try_into().unwrap()))
            .collect();

        let base = self
            .resolved
            .iter()
            .enumerate()
            .filter_map(|(scope, _)| ticks.get(scope * 2).copied())
            .filter(|&t| t > 0)
            .min()
            .unwrap_or(0);
        let to_us = |t: u64| (t.saturating_sub(base)) as f64 * self.period_ns as f64 / 1000.0;

        self.resolved
            .iter()
            .enumerate()
            .filter_map(|(scope, label)| {
                let begin = *ticks.get(scope * 2)?;
                let end = *ticks.get(scope * 2 + 1)?;
                if begin == 0 {
                    return None;
                }
                // An unwritten end reads as 0; report it as a zero-length
                // interval rather than letting it underflow into nonsense.
                let end = if end < begin { begin } else { end };
                Some((*label, to_us(begin), to_us(end)))
            })
            .collect()
    }

    /// Resolved bytes to `(label, microseconds)`, in recording order. Pairs
    /// that didn't advance are dropped: a backend may return zeroes for
    /// queries it couldn't satisfy.
    ///
    /// Prefer [`decode_intervals`](Self::decode_intervals) for anything that
    /// sums or compares scopes — see the overlap note there.
    pub fn decode(&self, bytes: &[u8]) -> Vec<(&'static str, f64)> {
        let ticks: Vec<u64> = bytes
            .chunks_exact(8)
            .map(|c| u64::from_le_bytes(c.try_into().unwrap()))
            .collect();

        self.resolved
            .iter()
            .enumerate()
            .filter_map(|(scope, label)| {
                let begin = ticks.get(scope * 2)?;
                let end = ticks.get(scope * 2 + 1)?;
                let delta = end.checked_sub(*begin).filter(|d| *d > 0)?;
                Some((*label, delta as f64 * self.period_ns as f64 / 1000.0))
            })
            .collect()
    }
}
