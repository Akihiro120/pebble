/// Generates a bitflags-style wrapper struct around a `wgpu` flags type, so
/// no `wgpu` type leaks into the public API.
macro_rules! bitflags_mirror {
    (
        $(#[$doc:meta])*
        pub struct $Name:ident => $Wgpu:ty {
            $(const $Flag:ident = $val:expr;)*
        }
    ) => {
        $(#[$doc])*
        #[derive(Copy, Clone, PartialEq, Eq, Hash)]
        pub struct $Name(u32);

        impl $Name {
            $(pub const $Flag: Self = Self($val);)*

            pub const fn empty() -> Self {
                Self(0)
            }

            pub const fn bits(self) -> u32 {
                self.0
            }

            pub const fn contains(self, other: Self) -> bool {
                (self.0 & other.0) == other.0
            }

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
    /// Which shader stages a bind group entry is visible to.
    pub struct ShaderStages => wgpu::ShaderStages {
        const NONE = 0;
        const VERTEX = 1 << 0;
        const FRAGMENT = 1 << 1;
        const COMPUTE = 1 << 2;
        const VERTEX_FRAGMENT = (1 << 0) | (1 << 1);
        const TASK = 1 << 3;
        const MESH = 1 << 4;
        const RAY_GENERATION = 1 << 5;
        const ANY_HIT = 1 << 6;
        const CLOSEST_HIT = 1 << 7;
        const MISS = 1 << 8;
    }
}

bitflags_mirror! {
    /// What a GPU buffer can be used for — passed to [`BufferBuilder::with_usage`](crate::graphics::pipeline::buffers::BufferBuilder::with_usage).
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
        const BLAS_INPUT = 1 << 10;
        const TLAS_INPUT = 1 << 11;
    }
}

bitflags_mirror! {
    /// What a GPU texture can be used for — passed to [`Texture::with_extra_usage`](crate::graphics::pipeline::textures::Texture::with_extra_usage).
    pub struct TextureUsages => wgpu::TextureUsages {
        const COPY_SRC = 1 << 0;
        const COPY_DST = 1 << 1;
        const TEXTURE_BINDING = 1 << 2;
        const STORAGE_BINDING = 1 << 3;
        const RENDER_ATTACHMENT = 1 << 4;
        const STORAGE_ATOMIC = 1 << 16;
        const TRANSIENT = 1 << 17;
    }
}

bitflags_mirror! {
    /// Which color channels a render pipeline writes — see [`ColorTargetState`](super::pipeline_state::ColorTargetState).
    pub struct ColorWrites => wgpu::ColorWrites {
        const RED = 1 << 0;
        const GREEN = 1 << 1;
        const BLUE = 1 << 2;
        const ALPHA = 1 << 3;
        const COLOR = (1 << 0) | (1 << 1) | (1 << 2);
        const ALL = (1 << 0) | (1 << 1) | (1 << 2) | (1 << 3);
    }
}

impl Default for ColorWrites {
    fn default() -> Self {
        Self::ALL
    }
}

/// Optional GPU capabilities that can be requested when the graphics backend is
/// initialized — see [`Backend::features`](super::render::Backend::features).
///
/// Support varies by platform and driver; requesting a feature the adapter
/// doesn't support is safe — it is dropped rather than failing device
/// creation. Query [`Backend::features`] to see what was actually granted.
///
/// This mirrors a curated subset of `wgpu::Features` — unlike the other flag
/// types in this module it isn't a 1:1 bit-for-bit copy, since `wgpu::Features`
/// is backed by a 128-bit value. Each flag is translated individually in the
/// `From` impl below instead.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct DeviceFeatures(u32);

impl DeviceFeatures {
    pub const ADDRESS_MODE_CLAMP_TO_BORDER: Self = Self(1 << 0);
    pub const TIMESTAMP_QUERY: Self = Self(1 << 1);
    pub const PIPELINE_STATISTICS_QUERY: Self = Self(1 << 2);
    pub const INDIRECT_FIRST_INSTANCE: Self = Self(1 << 3);
    pub const TEXTURE_COMPRESSION_BC: Self = Self(1 << 4);
    pub const TEXTURE_COMPRESSION_ETC2: Self = Self(1 << 5);
    pub const TEXTURE_COMPRESSION_ASTC: Self = Self(1 << 6);
    pub const POLYGON_MODE_LINE: Self = Self(1 << 7);
    pub const FLOAT32_FILTERABLE: Self = Self(1 << 8);
    pub const SHADER_F16: Self = Self(1 << 9);
    pub const SUBGROUP: Self = Self(1 << 10);
    pub const RAY_QUERY: Self = Self(1 << 11);
    pub const MESH_SHADER: Self = Self(1 << 12);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub const fn intersects(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }
}

impl core::ops::BitOr for DeviceFeatures {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitOrAssign for DeviceFeatures {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl From<DeviceFeatures> for wgpu::Features {
    fn from(value: DeviceFeatures) -> Self {
        let mut features = wgpu::Features::empty();
        let checks: &[(DeviceFeatures, wgpu::Features)] = &[
            (
                DeviceFeatures::ADDRESS_MODE_CLAMP_TO_BORDER,
                wgpu::Features::ADDRESS_MODE_CLAMP_TO_BORDER,
            ),
            (DeviceFeatures::TIMESTAMP_QUERY, wgpu::Features::TIMESTAMP_QUERY),
            (
                DeviceFeatures::PIPELINE_STATISTICS_QUERY,
                wgpu::Features::PIPELINE_STATISTICS_QUERY,
            ),
            (
                DeviceFeatures::INDIRECT_FIRST_INSTANCE,
                wgpu::Features::INDIRECT_FIRST_INSTANCE,
            ),
            (
                DeviceFeatures::TEXTURE_COMPRESSION_BC,
                wgpu::Features::TEXTURE_COMPRESSION_BC,
            ),
            (
                DeviceFeatures::TEXTURE_COMPRESSION_ETC2,
                wgpu::Features::TEXTURE_COMPRESSION_ETC2,
            ),
            (
                DeviceFeatures::TEXTURE_COMPRESSION_ASTC,
                wgpu::Features::TEXTURE_COMPRESSION_ASTC,
            ),
            (DeviceFeatures::POLYGON_MODE_LINE, wgpu::Features::POLYGON_MODE_LINE),
            (DeviceFeatures::FLOAT32_FILTERABLE, wgpu::Features::FLOAT32_FILTERABLE),
            (DeviceFeatures::SHADER_F16, wgpu::Features::SHADER_F16),
            (DeviceFeatures::SUBGROUP, wgpu::Features::SUBGROUP),
            (DeviceFeatures::RAY_QUERY, wgpu::Features::EXPERIMENTAL_RAY_QUERY),
            (DeviceFeatures::MESH_SHADER, wgpu::Features::EXPERIMENTAL_MESH_SHADER),
        ];
        for (flag, wgpu_flag) in checks {
            if value.contains(*flag) {
                features |= *wgpu_flag;
            }
        }
        features
    }
}

impl From<wgpu::Features> for DeviceFeatures {
    fn from(value: wgpu::Features) -> Self {
        let mut result = DeviceFeatures::empty();
        let checks: &[(wgpu::Features, DeviceFeatures)] = &[
            (
                wgpu::Features::ADDRESS_MODE_CLAMP_TO_BORDER,
                DeviceFeatures::ADDRESS_MODE_CLAMP_TO_BORDER,
            ),
            (wgpu::Features::TIMESTAMP_QUERY, DeviceFeatures::TIMESTAMP_QUERY),
            (
                wgpu::Features::PIPELINE_STATISTICS_QUERY,
                DeviceFeatures::PIPELINE_STATISTICS_QUERY,
            ),
            (
                wgpu::Features::INDIRECT_FIRST_INSTANCE,
                DeviceFeatures::INDIRECT_FIRST_INSTANCE,
            ),
            (
                wgpu::Features::TEXTURE_COMPRESSION_BC,
                DeviceFeatures::TEXTURE_COMPRESSION_BC,
            ),
            (
                wgpu::Features::TEXTURE_COMPRESSION_ETC2,
                DeviceFeatures::TEXTURE_COMPRESSION_ETC2,
            ),
            (
                wgpu::Features::TEXTURE_COMPRESSION_ASTC,
                DeviceFeatures::TEXTURE_COMPRESSION_ASTC,
            ),
            (wgpu::Features::POLYGON_MODE_LINE, DeviceFeatures::POLYGON_MODE_LINE),
            (wgpu::Features::FLOAT32_FILTERABLE, DeviceFeatures::FLOAT32_FILTERABLE),
            (wgpu::Features::SHADER_F16, DeviceFeatures::SHADER_F16),
            (wgpu::Features::SUBGROUP, DeviceFeatures::SUBGROUP),
            (wgpu::Features::EXPERIMENTAL_RAY_QUERY, DeviceFeatures::RAY_QUERY),
            (wgpu::Features::EXPERIMENTAL_MESH_SHADER, DeviceFeatures::MESH_SHADER),
        ];
        for (wgpu_flag, flag) in checks {
            if value.contains(*wgpu_flag) {
                result |= *flag;
            }
        }
        result
    }
}
