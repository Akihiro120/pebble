macro_rules! bitflags_mirror {
    (
        pub struct $Name:ident => $Wgpu:ty {
            $(const $Flag:ident = $val:expr;)*
        }
    ) => {
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
