use core::{
    scene::Camera,
    types::{Color, Vec2f, Vec3f, Vec4f},
    util::{angle2rad, any_as_u8_slice},
};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct ShadowConfig {
    pub cast_shadow: bool,
    pub size: Vec2f,
    pub offset: f32,
}

impl Default for ShadowConfig {
    fn default() -> Self {
        Self {
            cast_shadow: true,
            size: Vec2f::new(1024f32, 1024f32),
            offset: 0.01f32,
        }
    }
}

pub trait TLight {
    fn light_uniform(&self) -> Vec<u8>;
    fn shadow_uniform(&self) -> Vec<u8>;
    fn shadow_config(&self) -> &ShadowConfig;
}

pub enum Light {
    Direct(DirectLight),
    Spot(SpotLight),
    Point(PointLight),
}

impl TLight for Light {
    fn light_uniform(&self) -> Vec<u8> {
        match self {
            Light::Direct(d) => d.light_uniform(),
            Light::Spot(s) => s.light_uniform(),
            Light::Point(p) => p.light_uniform(),
        }
    }

    fn shadow_uniform(&self) -> Vec<u8> {
        match self {
            Light::Direct(d) => d.shadow_uniform(),
            Light::Spot(s) => s.shadow_uniform(),
            Light::Point(p) => p.shadow_uniform(),
        }
    }

    fn shadow_config(&self) -> &ShadowConfig {
        match self {
            Light::Direct(d) => d.shadow_config(),
            Light::Spot(s) => s.shadow_config(),
            Light::Point(p) => p.shadow_config(),
        }
    }
}

#[repr(C)]
struct DirectLightUniform {
    color: Vec3f,
    _a: f32,
    dir: Vec3f,
    _b: f32,
}

#[repr(C)]
struct PointLightUniform {
    color: Vec3f,
    _a: f32,
    pos: Vec3f,
    _b: f32,
}

#[repr(C)]
struct SpotLightUniform {
    color: Vec3f,
    _a: f32,
    pos: Vec3f,
    cutoff: f32,
    dir: Vec3f,
    cutoff_outer: f32,
}

pub struct DirectLight {
    color: Color,
    camera: Camera,
    shadow: ShadowConfig,
}

impl TLight for DirectLight {
    fn light_uniform(&self) -> Vec<u8> {
        let dir = (self.camera.to() - self.camera.from()).normalize();
        let u = DirectLightUniform {
            color: Vec3f::new(self.color.x, self.color.y, self.color.z),
            _a: 0f32,
            dir: dir,
            _b: 0f32,
        };
        any_as_u8_slice(&u).to_owned()
    }

    fn shadow_uniform(&self) -> Vec<u8> {
        self.camera.uniform_3d()
    }

    fn shadow_config(&self) -> &ShadowConfig {
        &self.shadow
    }
}

pub struct PointLight {
    color: Color,
    pos: Vec3f,
    camera: Camera,
    shadow: ShadowConfig,
}

impl TLight for PointLight {
    fn light_uniform(&self) -> Vec<u8> {
        let u = PointLightUniform {
            color: Vec3f::new(self.color.x, self.color.y, self.color.z),
            _a: 0f32,
            pos: self.pos,
            _b: 0f32,
        };
        any_as_u8_slice(&u).to_owned()
    }
    fn shadow_uniform(&self) -> Vec<u8> {
        self.camera.uniform_3d()
    }
    fn shadow_config(&self) -> &ShadowConfig {
        &self.shadow
    }
}

pub struct SpotLight {
    color: Color,
    pos: Vec3f,
    dir: Vec3f,
    cutoff: f32,
    cutoff_outer: f32,
    camera: Camera,
    shadow: ShadowConfig,
}

impl TLight for SpotLight {
    fn light_uniform(&self) -> Vec<u8> {
        let u = SpotLightUniform {
            color: Vec3f::new(self.color.x, self.color.y, self.color.z),
            _a: 0f32,
            pos: self.pos,
            cutoff: self.cutoff,
            dir: self.dir,
            cutoff_outer: self.cutoff_outer,
        };
        any_as_u8_slice(&u).to_owned()
    }

    fn shadow_uniform(&self) -> Vec<u8> {
        self.camera.uniform_3d()
    }

    fn shadow_config(&self) -> &ShadowConfig {
        &self.shadow
    }
}

struct SceneLightsInner {
    direct_light: Option<Arc<Light>>,
    extra_lights: Vec<Arc<Light>>,
    ambient: Color,
}

impl Default for SceneLightsInner {
    fn default() -> Self {
        Self {
            ambient: Vec4f::new(0.2f32, 0.2f32, 0.2f32, 1.0f32),
            extra_lights: vec![],
            direct_light: None,
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
        inner.direct_light = Some(Arc::new(Light::Direct(light)));
    }

    pub fn add_point_light(&self, light: PointLight) {
        let mut inner = self.inner.lock().unwrap();
        inner.extra_lights.push(Arc::new(Light::Point(light)));
    }
    pub fn add_spot_light(&self, light: SpotLight) {
        let mut inner = self.inner.lock().unwrap();
        inner.extra_lights.push(Arc::new(Light::Spot(light)));
    }

    pub fn has_direct_light(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.direct_light.is_some()
    }

    pub fn extra_lights(&self) -> Vec<Arc<Light>> {
        let inner = self.inner.lock().unwrap();
        inner.extra_lights.clone()
    }

    pub fn direct_light(&self) -> Arc<Light> {
        let inner = self.inner.lock().unwrap();
        inner.direct_light.clone().unwrap()
    }

    pub fn base_uniform(&self) -> Vec<u8> {
        let inner = self.inner.lock().unwrap();
        any_as_u8_slice(&inner.ambient).to_owned()
    }

    pub fn any_shadow(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        if let Some(d) = &inner.direct_light {
            if let Light::Direct(d) = d.as_ref() {
                if d.shadow.cast_shadow {
                    return true;
                }
            }
        }
        for e in &inner.extra_lights {
            match e.as_ref() {
                Light::Spot(s) => {
                    if s.shadow.cast_shadow {
                        return true;
                    }
                }
                Light::Point(p) => {
                    if p.shadow.cast_shadow {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }
}

pub struct DirectLightBuilder {
    color: Color,
    position: Vec3f,
    dir: Vec3f,
    shadow_rect: Vec4f,
    near: f32,
    far: f32,
    shadow: ShadowConfig,
}

impl DirectLightBuilder {
    pub fn new() -> Self {
        Self {
            color: Color::new(0.5f32, 0.5f32, 0.5f32, 1f32),
            position: Vec3f::new(0f32, 0f32, 0f32),
            dir: Vec3f::new(1f32, 0f32, 0f32),
            shadow_rect: Vec4f::new(-10f32, 10f32, 10f32, -10f32),
            near: 0.00001f32,
            far: 30f32,
            shadow: ShadowConfig::default(),
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

    pub fn cast_shadow(mut self, config: ShadowConfig) -> Self {
        self.shadow = config;
        self
    }

    pub fn build(mut self) -> DirectLight {
        let c = Camera::new();

        c.make_orthographic(self.shadow_rect, self.near, self.far);
        let to = self.position + self.dir;
        c.look_at(self.position, to, Vec3f::new(0f32, 1f32, 0f32));
        DirectLight {
            color: self.color,
            camera: c,
            shadow: self.shadow,
        }
    }
}

impl Default for DirectLightBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct PointLightBuilder {
    color: Color,
    position: Vec3f,
    shadow: ShadowConfig,
}

impl PointLightBuilder {
    pub fn new() -> Self {
        Self {
            color: Color::new(1f32, 1f32, 1f32, 1f32),
            position: Vec3f::zeros(),
            shadow: ShadowConfig::default(),
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

    pub fn cast_shadow(mut self, config: ShadowConfig) -> Self {
        self.shadow = config;
        self
    }

    pub fn build(self) -> PointLight {
        let c = Camera::new();
        // c.make_orthographic(self.shadow_rect, self.near, self.far);
        // let to = self.position + self.dir;
        // c.look_at(self.position, to, Vec3f::new(0f32, 1f32, 0f32));
        PointLight {
            color: self.color,
            pos: self.position,
            camera: c,
            shadow: self.shadow,
        }
    }
}

impl Default for PointLightBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SpotLightBuilder {
    color: Color,
    position: Vec3f,
    dir: Vec3f,
    cutoff: f32,
    cutoff_outer: f32,
    shadow: ShadowConfig,
}

impl SpotLightBuilder {
    pub fn new() -> Self {
        Self {
            color: Color::new(1f32, 1f32, 1f32, 1f32),
            position: Vec3f::zeros(),
            dir: Vec3f::new(0f32, -1f32, 0f32),
            cutoff: angle2rad(90f32),
            cutoff_outer: angle2rad(120f32),
            shadow: ShadowConfig::default(),
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

    pub fn cast_shadow(mut self, config: ShadowConfig) -> Self {
        self.shadow = config;
        self
    }

    pub fn cutoff(mut self, cutoff: f32, cutoff_outer: f32) -> Self {
        self.cutoff = cutoff;
        self.cutoff_outer = cutoff_outer;
        self
    }

    pub fn build(self) -> SpotLight {
        let c = Camera::new();
        // c.make_orthographic(self.shadow_rect, self.near, self.far);
        // let to = self.position + self.dir;
        // c.look_at(self.position, to, Vec3f::new(0f32, 1f32, 0f32));
        SpotLight {
            color: self.color,
            pos: self.position,
            dir: self.dir,
            cutoff: self.cutoff,
            cutoff_outer: self.cutoff_outer,
            camera: c,
            shadow: self.shadow,
        }
    }
}

impl Default for SpotLightBuilder {
    fn default() -> Self {
        Self::new()
    }
}
