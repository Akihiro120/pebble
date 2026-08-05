//! Hand-rolled bitflag mirrors for wgpu's flag-style types — small enough
//! (at most a couple dozen bits) that pulling in the `bitflags` crate isn't
//! worth it. Each mirrors its `wgpu` counterpart's bit layout exactly, so
//! converting into wgpu is just `from_bits_truncate`.

macro_rules! bitflags_mirror {
    (
        $(#[$outer:meta])*
        pub struct $Name:ident => $Wgpu:ty {
            $($(#[$inner:meta])* const $Flag:ident = $val:expr;)*
        }
    ) => {
        $(#[$outer])*
        #[derive(Copy, Clone, PartialEq, Eq, Hash)]
        pub struct $Name(u32);

        impl $Name {
            $($(#[$inner])* pub const $Flag: Self = Self($val);)*

            /// No flags set.
            pub const fn empty() -> Self {
                Self(0)
            }

            pub const fn bits(self) -> u32 {
                self.0
            }

            /// Whether every flag set in `other` is also set in `self`.
            pub const fn contains(self, other: Self) -> bool {
                (self.0 & other.0) == other.0
            }

            /// Whether `self` and `other` share any set flag.
            pub const fn intersects(self, other: Self) -> bool {
                (self.0 & other.0) != 0
            }
        }

        impl core::ops::BitOr for $Name {
            type Output = Self;
            fn bitor(self, rhs: Self) -> Self {
                Self(self.0 | rhs.0)
            }
        }

        impl core::ops::BitOrAssign for $Name {
            fn bitor_assign(&mut self, rhs: Self) {
                self.0 |= rhs.0;
            }
        }

        impl From<$Name> for $Wgpu {
            fn from(value: $Name) -> Self {
                <$Wgpu>::from_bits_truncate(value.0)
            }
        }
    };
}

bitflags_mirror! {
    /// Which shader stage(s) a binding is visible from — mirrors
    /// `wgpu::ShaderStages` bit-for-bit.
    pub struct ShaderStages => wgpu::ShaderStages {
        /// Not visible from any shader stage.
        const NONE = 0;
        /// Visible from the vertex shader of a render pipeline.
        const VERTEX = 1 << 0;
        /// Visible from the fragment shader of a render pipeline.
        const FRAGMENT = 1 << 1;
        /// Visible from the compute shader of a compute pipeline.
        const COMPUTE = 1 << 2;
        /// Visible from the vertex and fragment shaders of a render pipeline.
        const VERTEX_FRAGMENT = (1 << 0) | (1 << 1);
        /// Visible from the task shader of a mesh pipeline.
        const TASK = 1 << 3;
        /// Visible from the mesh shader of a mesh pipeline.
        const MESH = 1 << 4;
        /// Visible from the ray generation shader of a ray tracing pipeline.
        const RAY_GENERATION = 1 << 5;
        /// Visible from the ray any-hit shader of a ray tracing pipeline.
        const ANY_HIT = 1 << 6;
        /// Visible from the ray closest-hit shader of a ray tracing pipeline.
        const CLOSEST_HIT = 1 << 7;
        /// Visible from the ray miss shader of a ray tracing pipeline.
        const MISS = 1 << 8;
    }
}

bitflags_mirror! {
    /// Allowed usages of a buffer — mirrors `wgpu::BufferUsages` bit-for-bit.
    pub struct BufferUsages => wgpu::BufferUsages {
        const MAP_READ = 1 << 0;
        const MAP_WRITE = 1 << 1;
        const COPY_SRC = 1 << 2;
        const COPY_DST = 1 << 3;
        const INDEX = 1 << 4;
        const VERTEX = 1 << 5;
        const UNIFORM = 1 << 6;
        const STORAGE = 1 << 7;
        const INDIRECT = 1 << 8;
        const QUERY_RESOLVE = 1 << 9;
        /// Allows a buffer to be used as input for a bottom level acceleration structure build.
        const BLAS_INPUT = 1 << 10;
        /// Allows a buffer to be used as input for a top level acceleration structure build.
        const TLAS_INPUT = 1 << 11;
    }
}

bitflags_mirror! {
    /// Allowed usages of a texture — mirrors `wgpu::TextureUsages` bit-for-bit.
    pub struct TextureUsages => wgpu::TextureUsages {
        const COPY_SRC = 1 << 0;
        const COPY_DST = 1 << 1;
        const TEXTURE_BINDING = 1 << 2;
        const STORAGE_BINDING = 1 << 3;
        const RENDER_ATTACHMENT = 1 << 4;
        /// Allows a texture to be used with image atomics. Requires a native-only feature.
        const STORAGE_ATOMIC = 1 << 16;
        /// The contents of this texture will not be reused in another pass.
        const TRANSIENT = 1 << 17;
    }
}

bitflags_mirror! {
    /// Which color channels a render pipeline writes — mirrors
    /// `wgpu::ColorWrites` bit-for-bit.
    pub struct ColorWrites => wgpu::ColorWrites {
        const RED = 1 << 0;
        const GREEN = 1 << 1;
        const BLUE = 1 << 2;
        const ALPHA = 1 << 3;
        /// Red, green, and blue, but not alpha.
        const COLOR = (1 << 0) | (1 << 1) | (1 << 2);
        /// Every channel.
        const ALL = (1 << 0) | (1 << 1) | (1 << 2) | (1 << 3);
    }
}

impl Default for ColorWrites {
    /// Matches `wgpu::ColorWrites`'s default — every channel.
    fn default() -> Self {
        Self::ALL
    }
}
