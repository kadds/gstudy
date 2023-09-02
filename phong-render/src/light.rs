use core::{
    scene::Camera,
    types::{Color, Vec3f, Vec4f},
    util::any_as_u8_slice,
};
use std::{
    io::Write,
    sync::{Arc, Mutex},
};

#[repr(C)]
struct DirectLightUniform {
    color: Vec3f,
    _a: f32,
    dir: Vec3f,
    _b: f32,
}

pub struct DirectLight {
    color: Color,
    camera: Arc<Camera>,
    pub(crate) cast_shadow: bool,
}

impl DirectLight {
    fn uniform(&self) -> DirectLightUniform {
        let dir = (self.camera.to() - self.camera.from()).normalize();
        DirectLightUniform {
            color: Vec3f::new(self.color.x, self.color.y, self.color.z),
            _a: 0f32,
            dir: dir,
            _b: 0f32,
        }
    }
}

pub struct DirectLightBuilder {
    color: Color,
    position: Vec3f,
    dir: Vec3f,
    shadow_rect: Vec4f,
    near: f32,
    far: f32,
    cast_shadow: bool,
    cast_shadow_distance: f32,
}

impl DirectLightBuilder {
    pub fn new() -> Self {
        Self {
            color: Color::new(0.5f32, 0.5f32, 0.5f32, 1f32),
            position: Vec3f::new(0f32, 0f32, 0f32),
            dir: Vec3f::new(1f32, 0f32, 0f32),
            shadow_rect: Vec4f::new(-10f32, -10f32, 10f32, 10f32),
            near: 0.1f32,
            far: 150f32,
            cast_shadow: false,
            cast_shadow_distance: 1000f32,
        }
    }
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn position(mut self, position: Vec3f) -> Self {
        self.position = position;
        self
    }

    pub fn direction(mut self, direction: Vec3f) -> Self {
        self.dir = direction.normalize();
        self
    }

    pub fn cast_shadow(mut self, cast: bool) -> Self {
        self.cast_shadow = cast;
        self
    }

    pub fn cast_shadow_distance(mut self, distance: f32) -> Self {
        self.cast_shadow_distance = distance;
        self
    }

    pub fn build(self) -> DirectLight {
        let c = Camera::new();
        c.make_orthographic(self.shadow_rect, self.near, self.far);
        let to = self.position + self.dir * self.cast_shadow_distance;
        c.look_at(self.position, to, Vec3f::new(0f32, 1f32, 0f32));
        DirectLight {
            color: self.color,
            camera: Arc::new(c),
            cast_shadow: self.cast_shadow,
        }
    }
}

impl Default for DirectLightBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct PointLight {
    color: Color,
    camera: Camera,
}

pub struct SpotLight {
    color: Color,
    camera: Camera,
}
struct SceneLightsInner {
    direct_light: Option<DirectLight>,
    point_light: Vec<PointLight>,
    spot_light: Vec<SpotLight>,
    dirty: bool,
    ambient: Color,
    base_uniform: Vec<u8>,
    add_uniform: Vec<Vec<u8>>,
}

impl Default for SceneLightsInner {
    fn default() -> Self {
        Self {
            dirty: true,
            ambient: Vec4f::new(0.2f32, 0.2f32, 0.2f32, 1.0f32),
            direct_light: None,
            point_light: vec![],
            spot_light: vec![],
            base_uniform: vec![],
            add_uniform: vec![],
        }
    }
}

pub struct SceneLights {
    inner: Mutex<SceneLightsInner>,
}

impl Default for SceneLights {
    fn default() -> Self {
        Self {
            inner: Mutex::new(SceneLightsInner::default()),
        }
    }
}

impl SceneLights {
    pub fn set_ambient(&self, ambient: Color) {
        let mut inner = self.inner.lock().unwrap();
        inner.ambient = ambient;
    }
    pub fn set_direct_light(&self, light: DirectLight) {
        let mut inner = self.inner.lock().unwrap();
        inner.direct_light = Some(light);
        inner.dirty = true;
    }

    pub fn has_direct_light(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.direct_light.is_some()
    }

    pub fn extra_lights(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        let n = inner.point_light.len() + inner.spot_light.len();
        n
    }

    pub fn direct_light_cast_shadow(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        inner
            .direct_light
            .as_ref()
            .map(|v| v.cast_shadow)
            .unwrap_or_default()
    }

    pub fn direct_light_camera(&self) -> Arc<Camera> {
        let inner = self.inner.lock().unwrap();
        inner
            .direct_light
            .as_ref()
            .map(|v| v.camera.clone())
            .unwrap()
    }

    fn update_uniform_inner(inner: &mut SceneLightsInner) {
        let mut base_uniform = vec![];
        base_uniform.write_all(any_as_u8_slice(&inner.ambient));

        if let Some(dir) = &inner.direct_light {
            base_uniform.write_all(any_as_u8_slice(&dir.uniform()));
        }
        inner.base_uniform = base_uniform;
    }

    pub fn base_uniform(&self) -> Vec<u8> {
        let mut inner = self.inner.lock().unwrap();
        if inner.dirty {
            Self::update_uniform_inner(&mut inner);
        }
        inner.base_uniform.clone()
    }
}
