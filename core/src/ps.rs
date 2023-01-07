use std::{any::Any, io::Cursor, sync::Arc};

use crate::util::any_as_u8_slice;

use super::context::RContext;

#[derive(Debug, Copy, Clone)]
pub enum Topology {
    PointList,
    LineList,
    LineStrip,
    TriangleList,
    TriangleStrip,
}

impl Default for Topology {
    fn default() -> Self {
        Self::TriangleList
    }
}

#[derive(Debug, Copy, Clone)]
pub enum CullFace {
    None,
    Front,
    Back,
}

impl Default for CullFace {
    fn default() -> Self {
        Self::Back
    }
}

#[derive(Debug, Copy, Clone)]
pub enum PolygonMode {
    Fill,
    Line,
    Point,
}

impl Default for PolygonMode {
    fn default() -> Self {
        Self::Fill
    }
}

#[derive(Debug, Copy, Clone)]
pub enum BlendFactor {
    /// 0.0
    Zero = 0,
    /// 1.0
    One = 1,
    /// S.component
    Src = 2,
    /// 1.0 - S.component
    OneMinusSrc = 3,
    /// S.alpha
    SrcAlpha = 4,
    /// 1.0 - S.alpha
    OneMinusSrcAlpha = 5,
    /// D.component
    Dst = 6,
    /// 1.0 - D.component
    OneMinusDst = 7,
    /// D.alpha
    DstAlpha = 8,
    /// 1.0 - D.alpha
    OneMinusDstAlpha = 9,
    /// min(S.alpha, 1.0 - D.alpha)
    SrcAlphaSaturated = 10,
    /// Constant
    Constant = 11,
    /// 1.0 - Constant
    OneMinusConstant = 12,
}

#[derive(Debug, Copy, Clone)]
pub enum BlendOperation {
    /// Src + Dst
    Add = 0,
    /// Src - Dst
    Subtract = 1,
    /// Dst - Src
    ReverseSubtract = 2,
    /// min(Src, Dst)
    Min = 3,
    /// max(Src, Dst)
    Max = 4,
}

#[derive(Debug, Clone)]
pub struct BlendComponent {
    pub src_factor: BlendFactor,
    pub dst_factor: BlendFactor,
    pub operation: BlendOperation,
}

impl BlendComponent {
    pub fn as_key(&self) -> u32 {
        self.src_factor as u32 + ((self.dst_factor as u32) << 4) + ((self.operation as u32) << 8)
    }
}

#[derive(Debug, Clone)]
pub struct BlendState {
    pub color: BlendComponent,
    pub alpha: BlendComponent,
}

impl BlendState {
    pub fn default_append_blender() -> Self {
        Self {
            color: BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            },
            alpha: BlendComponent {
                src_factor: BlendFactor::OneMinusDstAlpha,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::Add,
            },
        }
    }
    pub fn default_gltf_blender() -> Self {
        Self {
            color: BlendComponent {
                src_factor: BlendFactor::SrcAlpha,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            },
            alpha: BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::Zero,
                operation: BlendOperation::Add,
            },
        }
    }

    pub fn as_key(&self) -> u64 {
        let a = self.color.as_key();
        let b = self.alpha.as_key();
        let mut result = ((a as u64) << 32) | (b as u64);
        let slice = unsafe {
            ::std::slice::from_raw_parts_mut(
                (&mut result as *mut u64) as *mut u8,
                ::std::mem::size_of::<u64>(),
            )
        };
        let mut c = Cursor::new(slice);

        (murmur3::murmur3_32(&mut c, 456).unwrap() % (u16::MAX as u32 - 1)) as u64
    }
}

#[derive(Debug)]
pub enum CompareFunction {
    /// Function never passes
    Never = 1,
    /// Function passes if new value less than existing value
    Less = 2,
    /// Function passes if new value is equal to existing value. When using
    /// this compare function, make sure to mark your Vertex Shader's `@builtin(position)`
    /// output as `@invariant` to prevent artifacting.
    Equal = 3,
    /// Function passes if new value is less than or equal to existing value
    LessEqual = 4,
    /// Function passes if new value is greater than existing value
    Greater = 5,
    /// Function passes if new value is not equal to existing value. When using
    /// this compare function, make sure to mark your Vertex Shader's `@builtin(position)`
    /// output as `@invariant` to prevent artifacting.
    NotEqual = 6,
    /// Function passes if new value is greater than or equal to existing value
    GreaterEqual = 7,
    /// Function always passes
    Always = 8,
}

pub enum StencilOperation {
    /// Keep stencil value unchanged.
    Keep = 0,
    /// Set stencil value to zero.
    Zero = 1,
    /// Replace stencil value with value provided in most recent call to
    /// [`RenderPass::set_stencil_reference`][RPssr].
    ///
    /// [RPssr]: ../wgpu/struct.RenderPass.html#method.set_stencil_reference
    Replace = 2,
    /// Bitwise inverts stencil value.
    Invert = 3,
    /// Increments stencil value by one, clamping on overflow.
    IncrementClamp = 4,
    /// Decrements stencil value by one, clamping on underflow.
    DecrementClamp = 5,
    /// Increments stencil value by one, wrapping on overflow.
    IncrementWrap = 6,
    /// Decrements stencil value by one, wrapping on underflow.
    DecrementWrap = 7,
}

impl Default for StencilOperation {
    fn default() -> Self {
        Self::Keep
    }
}

#[derive(Debug)]
pub struct DepthDescriptor {
    depth_read_enabled: bool,
    depth_write_enabled: bool,
    compare: CompareFunction,
}

impl Default for DepthDescriptor {
    fn default() -> Self {
        Self {
            depth_write_enabled: true,
            depth_read_enabled: true,
            compare: CompareFunction::Always,
        }
    }
}

impl DepthDescriptor {
    pub fn with_compare(mut self, compare: CompareFunction) -> Self {
        self.compare = compare;
        self
    }
}

#[derive(Debug, Clone)]
pub struct PrimitiveStateDescriptor {
    topology: Topology,
    cull_face: CullFace,
    polygon_mode: PolygonMode,
}

impl Default for PrimitiveStateDescriptor {
    fn default() -> Self {
        Self {
            topology: Topology::TriangleList,
            cull_face: CullFace::Back,
            polygon_mode: PolygonMode::Fill,
        }
    }
}

impl PrimitiveStateDescriptor {
    pub fn with_topology(mut self, topology: Topology) -> Self {
        self.topology = topology;
        self
    }

    pub fn with_cull_face(mut self, face: CullFace) -> Self {
        self.cull_face = face;
        self
    }

    pub fn with_polygon_mode(mut self, mode: PolygonMode) -> Self {
        self.polygon_mode = mode;
        self
    }

    // topology 000
    // cull_face 00
    // polygon_mode 00
    pub fn as_key(&self) -> u64 {
        let mut val = self.topology as u64;
        val <<= 3;
        val |= self.cull_face as u64;
        val <<= 2;
        val |= self.polygon_mode as u64;
        val
    }

    pub fn topology(&self) -> Topology {
        self.topology
    }

    pub fn cull_face(&self) -> CullFace {
        self.cull_face
    }

    pub fn polygon(&self) -> PolygonMode {
        self.polygon_mode
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PipelineStateObject(u64);

impl PipelineStateObject {
    pub fn id(&self) -> u64 {
        self.0
    }
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

#[derive(Debug, Default)]
pub struct PipelineStateBuilder {
    blend: Option<BlendState>,
    depth: DepthDescriptor,
    primitive: PrimitiveStateDescriptor,
    topology: Topology,
    cull: CullFace,
    polygon: PolygonMode,
}
