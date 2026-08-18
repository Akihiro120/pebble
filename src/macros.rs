/// Unwraps an `Option`, returning from the enclosing function if it's
/// `None` — collapses the `let Some(x) = expr else { return };` pattern
/// that shows up constantly in systems (which return `()`, so the `?`
/// operator isn't an option the way it would be in a function returning
/// `Option`/`Result`).
///
/// ```rust,ignore
/// let material = or_return!(materials.get(handle));
/// let material = or_return!(materials.get(handle), return None); // custom return value
/// ```
#[macro_export]
macro_rules! or_return {
    ($expr:expr) => {
        match $expr {
            ::core::option::Option::Some(value) => value,
            ::core::option::Option::None => return,
        }
    };
    ($expr:expr, $ret:expr) => {
        match $expr {
            ::core::option::Option::Some(value) => value,
            ::core::option::Option::None => return $ret,
        }
    };
}

/// The "look up, then bind" chain every draw call repeats: `$materials.get($handle)`,
/// then `set_pipeline` + `set_bind_group(0, ...)` on `$pass`. Returns from
/// the enclosing function (via [`or_return!`]) if the lookup isn't ready
/// yet — an asset that hasn't finished uploading is a normal, common case
/// (a couple frames on load), not a bug.
///
/// Evaluates to the looked-up `&GPUMaterial`, so callers that also need
/// `.update(name, data)` on it (e.g. a per-frame camera uniform) can still
/// bind that:
///
/// ```rust,ignore
/// let material = bind_mat!(render_pass, materials, material_handle);
/// ```
#[macro_export]
macro_rules! bind_mat {
    ($pass:expr, $materials:expr, $handle:expr) => {{
        let material = $crate::or_return!($materials.get($handle));
        $pass.set_pipeline(&material.pipeline);
        $pass.set_bind_group(0, &material.bind_group, &[]);
        material
    }};
}

/// Same as [`bind_mat!`], for a `ComputePass` + [`Compute`](crate::graphics::pipeline::compute::Compute)
/// instead of a `RenderPass` + [`Material`](crate::graphics::pipeline::material::Material).
///
/// ```rust,ignore
/// let compute = bind_comp!(compute_pass, computes, compute_handle);
/// ```
#[macro_export]
macro_rules! bind_comp {
    ($pass:expr, $computes:expr, $handle:expr) => {{
        let compute = $crate::or_return!($computes.get($handle));
        $pass.set_pipeline(&compute.pipeline);
        $pass.set_bind_group(0, &compute.bind_group, &[]);
        compute
    }};
}

/// Looks up `$meshes.get($handle)` and draws it — sets the vertex/index
/// buffers and calls `draw_indexed`, defaulting the instance range to `0..1`
/// (pass a fourth argument for instanced draws). Returns from the enclosing
/// function (via [`or_return!`]) if the mesh isn't ready yet.
///
/// ```rust,ignore
/// draw_mesh!(render_pass, meshes, mesh_handle);
/// draw_mesh!(render_pass, meshes, mesh_handle, 0..enemy_count);
/// ```
#[macro_export]
macro_rules! draw_mesh {
    ($pass:expr, $meshes:expr, $handle:expr) => {
        $crate::draw_mesh!($pass, $meshes, $handle, 0..1)
    };
    ($pass:expr, $meshes:expr, $handle:expr, $instances:expr) => {{
        let mesh = $crate::or_return!($meshes.get($handle));
        $pass.set_vertex_buffer(0, &mesh.vertex_buffer);
        $pass.set_index_buffer(&mesh.index_buffer, $crate::graphics::types::IndexFormat::Uint32);
        $pass.draw_indexed(0..mesh.index_count, 0, $instances);
    }};
}

#[cfg(test)]
mod tests {
    use crate::graphics::types::IndexFormat;

    struct Assets<T>(Option<T>);

    impl<T> Assets<T> {
        fn get(&self, _handle: u32) -> Option<&T> {
            self.0.as_ref()
        }
    }

    struct GPUMaterial {
        pipeline: &'static str,
        bind_group: &'static str,
    }

    #[derive(Default)]
    struct RecordingPass {
        pipeline: Option<&'static str>,
        bind_group: Option<&'static str>,
        vertex_buffer: Option<&'static str>,
        index_buffer: Option<(&'static str, IndexFormat)>,
        drawn: Option<(std::ops::Range<u32>, i32, std::ops::Range<u32>)>,
    }

    impl RecordingPass {
        fn set_pipeline(&mut self, pipeline: &&'static str) {
            self.pipeline = Some(pipeline);
        }

        fn set_bind_group(&mut self, _index: u32, bind_group: &&'static str, _offsets: &[u32]) {
            self.bind_group = Some(bind_group);
        }

        fn set_vertex_buffer(&mut self, _slot: u32, buffer: &&'static str) {
            self.vertex_buffer = Some(buffer);
        }

        fn set_index_buffer(&mut self, buffer: &&'static str, format: IndexFormat) {
            self.index_buffer = Some((buffer, format));
        }

        fn draw_indexed(&mut self, indices: std::ops::Range<u32>, base_vertex: i32, instances: std::ops::Range<u32>) {
            self.drawn = Some((indices, base_vertex, instances));
        }
    }

    fn bind_missing(pass: &mut RecordingPass) {
        let materials: Assets<GPUMaterial> = Assets(None);
        bind_mat!(pass, materials, 0u32);
    }

    #[test]
    fn bind_mat_returns_early_when_material_missing() {
        let mut pass = RecordingPass::default();
        bind_missing(&mut pass);
        assert!(pass.pipeline.is_none());
        assert!(pass.bind_group.is_none());
    }

    fn bind_present(pass: &mut RecordingPass) {
        let materials = Assets(Some(GPUMaterial { pipeline: "pipeline", bind_group: "bind_group" }));
        let material = bind_mat!(pass, materials, 0u32);
        assert_eq!(material.bind_group, "bind_group");
    }

    #[test]
    fn bind_mat_sets_pipeline_and_bind_group() {
        let mut pass = RecordingPass::default();
        bind_present(&mut pass);
        assert_eq!(pass.pipeline, Some("pipeline"));
        assert_eq!(pass.bind_group, Some("bind_group"));
    }

    struct GPUMesh {
        vertex_buffer: &'static str,
        index_buffer: &'static str,
        index_count: u32,
    }

    #[test]
    fn draw_mesh_defaults_to_a_single_instance() {
        let mut pass = RecordingPass::default();
        let meshes = Assets(Some(GPUMesh { vertex_buffer: "vbo", index_buffer: "ibo", index_count: 6 }));
        draw_mesh!(pass, meshes, 0u32);

        assert_eq!(pass.vertex_buffer, Some("vbo"));
        assert_eq!(pass.index_buffer, Some(("ibo", IndexFormat::Uint32)));
        assert_eq!(pass.drawn, Some((0..6, 0, 0..1)));
    }

    #[test]
    fn draw_mesh_accepts_an_explicit_instance_range() {
        let mut pass = RecordingPass::default();
        let meshes = Assets(Some(GPUMesh { vertex_buffer: "vbo", index_buffer: "ibo", index_count: 6 }));
        draw_mesh!(pass, meshes, 0u32, 0..12);

        assert_eq!(pass.drawn, Some((0..6, 0, 0..12)));
    }

    #[test]
    fn draw_mesh_returns_early_when_missing() {
        let mut pass = RecordingPass::default();
        let meshes: Assets<GPUMesh> = Assets(None);
        draw_mesh!(pass, meshes, 0u32);

        assert!(pass.drawn.is_none());
    }
}
