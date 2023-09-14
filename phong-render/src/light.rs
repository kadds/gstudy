use core::{
    scene::Camera,
    types::{Color, Mat4x4f, Vec2f, Vec3f, Vec4f},
    util::{angle2rad, any_as_u8_slice},
};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct ShadowConfig {
    pub cast_shadow: bool,
    pub size: Vec2f,
    pub bias_factor: f32,
    pub pcf: bool,
}

impl Default for ShadowConfig {
    fn default() -> Self {
        Self {
            cast_shadow: false,
            size: Vec2f::new(1024f32, 1024f32),
            bias_factor: 1f32,
            pcf: false,
        }
    }
}

pub trait TLight {
    fn light_cameras(&self) -> &[Camera];
    fn light_uniform(&self) -> Vec<u8>;
    fn light_uniform_len(&self) -> usize;
    fn shadow_uniform(&self) -> Vec<u8>;
    fn shadow_config(&self) -> &ShadowConfig;
}

pub enum Light {
    Direct(DirectLight),
    Spot(SpotLight),
    Point(PointLight),
}

impl TLight for Light {
    fn light_cameras(&self) -> &[Camera] {
        match self {
            Light::Direct(d) => d.light_cameras(),
            Light::Spot(s) => s.light_cameras(),
            Light::Point(p) => p.light_cameras(),
        }
    }

    fn light_uniform(&self) -> Vec<u8> {
        match self {
            Light::Direct(d) => d.light_uniform(),
            Light::Spot(s) => s.light_uniform(),
            Light::Point(p) => p.light_uniform(),
        }
    }

    fn light_uniform_len(&self) -> usize {
        match self {
            Light::Direct(d) => d.light_uniform_len(),
            Light::Spot(s) => s.light_uniform_len(),
            Light::Point(p) => p.light_uniform_len(),
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
pub struct Attenuation {
    pub constant: f32,
    pub linear: f32,
    pub exp: f32,
    pub clip_distance: f32,
}

impl Default for Attenuation {
    fn default() -> Self {
        Self {
            constant: 1.0f32,
            linear: 0.25f32,
            exp: 0.045f32,
            clip_distance: 1000f32,
        }
    }
}

#[repr(C)]
struct DirectLightUniform {
    color: Vec3f,
    size_x: f32,
    dir: Vec3f,
    size_y: f32,
    vp: Mat4x4f,
    attenuation: Vec4f,
    intensity: f32,
    bias_factor: f32,
    _a0: f32,
    _a1: f32,
}

#[repr(C)]
struct PointLightUniform {
    color: Vec3f,
    size_x: f32,
    pos: Vec3f,
    size_y: f32,
    vp: Mat4x4f,
    attenuation: Vec4f,
    intensity: f32,
    bias_factor: f32,
    _a0: f32,
    _a1: f32,
}

#[repr(C)]
struct SpotLightUniform {
    vp: Mat4x4f,
    color: Vec3f,
    size_x: f32,
    pos: Vec3f,
    size_y: f32,
    dir: Vec3f,
    bias_factor: f32,
    cutoff: f32,
    cutoff_outer: f32,
    placement: f32,
    intensity: f32,
    attenuation: Vec4f,
}

#[repr(C)]
pub struct BaseLightUniform {
    ambient: Vec4f,
}

impl BaseLightUniform {
    pub fn as_bytes(&self) -> &[u8] {
        any_as_u8_slice(self)
    }
}

pub struct DirectLight {
    color: Color,
    camera: [Camera; 1],
    shadow: ShadowConfig,
    attenuation: Attenuation,
    intensity: f32,
}

impl TLight for DirectLight {
    fn light_cameras(&self) -> &[Camera] {
        &self.camera
    }

    fn light_uniform(&self) -> Vec<u8> {
        let camera = &self.camera[0];
        let dir = (camera.to() - camera.from()).normalize();
        let u = DirectLightUniform {
            color: Vec3f::new(self.color.x, self.color.y, self.color.z),
            size_x: self.shadow.size.x,
            dir,
            size_y: self.shadow.size.y,
            vp: camera.vp(),
            attenuation: Vec4f::new(
                self.attenuation.constant,
                self.attenuation.linear,
                self.attenuation.exp,
                self.attenuation.clip_distance,
            ),
            intensity: self.intensity,
            bias_factor: self.shadow.bias_factor,
            _a0: 0f32,
            _a1: 0f32,
        };
        any_as_u8_slice(&u).to_owned()
    }
    fn light_uniform_len(&self) -> usize {
        std::mem::size_of::<DirectLightUniform>()
    }

    fn shadow_uniform(&self) -> Vec<u8> {
        self.camera[0].uniform_shadow_3d()
    }

    fn shadow_config(&self) -> &ShadowConfig {
        &self.shadow
    }
}

pub struct PointLight {
    color: Color,
    pos: Vec3f,
    camera: [Camera; 6],
    shadow: ShadowConfig,
    attenuation: Attenuation,
    intensity: f32,
}

impl TLight for PointLight {
    fn light_cameras(&self) -> &[Camera] {
        &self.camera
    }

    fn light_uniform(&self) -> Vec<u8> {
        let camera = &self.camera[0];
        let u = PointLightUniform {
            color: Vec3f::new(self.color.x, self.color.y, self.color.z),
            size_x: self.shadow.size.x,
            pos: self.pos,
            size_y: self.shadow.size.y,
            vp: camera.vp(),
            attenuation: Vec4f::new(
                self.attenuation.constant,
                self.attenuation.linear,
                self.attenuation.exp,
                self.attenuation.clip_distance,
            ),
            intensity: self.intensity,
            bias_factor: self.shadow.bias_factor,
            _a0: 0f32,
            _a1: 0f32,
        };
        any_as_u8_slice(&u).to_owned()
    }
    fn light_uniform_len(&self) -> usize {
        std::mem::size_of::<PointLightUniform>()
    }
    fn shadow_uniform(&self) -> Vec<u8> {
        self.camera[0].uniform_shadow_3d()
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
    camera: [Camera; 1],
    shadow: ShadowConfig,
    attenuation: Attenuation,
    intensity: f32,
}

impl TLight for SpotLight {
    fn light_cameras(&self) -> &[Camera] {
        &self.camera
    }

    fn light_uniform(&self) -> Vec<u8> {
        let camera = &self.camera[0];
        let u = SpotLightUniform {
            color: Vec3f::new(self.color.x, self.color.y, self.color.z),
            size_x: self.shadow.size.x,
            size_y: self.shadow.size.y,
            pos: self.pos,
            cutoff: self.cutoff,
            dir: self.dir,
            cutoff_outer: self.cutoff_outer,
            vp: camera.vp(),
            bias_factor: self.shadow.bias_factor,
            intensity: self.intensity,
            placement: 0f32,
            attenuation: Vec4f::new(
                self.attenuation.constant,
                self.attenuation.linear,
                self.attenuation.exp,
                self.attenuation.clip_distance,
            ),
        };
        any_as_u8_slice(&u).to_owned()
    }

    fn light_uniform_len(&self) -> usize {
        std::mem::size_of::<SpotLightUniform>()
    }

    fn shadow_uniform(&self) -> Vec<u8> {
        self.camera[0].uniform_shadow_3d()
    }

    fn shadow_config(&self) -> &ShadowConfig {
        &self.shadow
    }
}

struct SceneLightsInner {
    direct_light: Option<Arc<Light>>,
    extra_lights: Vec<Arc<Light>>,
    base: Arc<Mutex<BaseLightUniform>>,
}

impl Default for SceneLightsInner {
    fn default() -> Self {
        Self {
            extra_lights: vec![],
            direct_light: None,
            base: Arc::new(Mutex::new(BaseLightUniform {
                ambient: Vec4f::new(0.2f32, 0.2f32, 0.2f32, 1.0f32),
            })),
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
        let inner = self.inner.lock().unwrap();
        let mut base = inner.base.lock().unwrap();
        base.ambient = ambient;
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

    pub fn direct_light(&self) -> Option<Arc<Light>> {
        let inner = self.inner.lock().unwrap();
        inner.direct_light.clone()
    }

    pub fn base_uniform_len(&self) -> usize {
        std::mem::size_of::<BaseLightUniform>()
    }

    pub fn base_uniform(&self) -> Arc<Mutex<BaseLightUniform>> {
        let inner = self.inner.lock().unwrap();
        inner.base.clone()
    }

    pub fn any_light(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        if let Some(d) = &inner.direct_light {
            if let Light::Direct(d) = d.as_ref() {
                return true;
            }
        }
        for e in &inner.extra_lights {
            match e.as_ref() {
                Light::Spot(s) => {
                    return true;
                }
                Light::Point(p) => {
                    return true;
                }
                _ => {}
            }
        }
        false
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
    attenuation: Attenuation,
    intensity: f32,
}

impl DirectLightBuilder {
    pub fn new() -> Self {
        Self {
            color: Color::new(0.5f32, 0.5f32, 0.5f32, 1f32),
            position: Vec3f::new(0f32, 0f32, 0f32),
            dir: Vec3f::new(1f32, 0f32, 0f32),
            shadow_rect: Vec4f::new(-5f32, -5f32, 5f32, 5f32),
            near: 0.0001f32,
            far: 12f32,
            shadow: ShadowConfig::default(),
            attenuation: Attenuation::default(),
            intensity: 1f32,
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

    pub fn attenuation(mut self, attenuation: Attenuation) -> Self {
        self.attenuation = attenuation;
        self
    }

    pub fn intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity;
        self
    }

    pub fn build(self) -> DirectLight {
        let c = Camera::new();

        c.make_orthographic(self.shadow_rect, self.near, self.far);
        // let to = self.position + self.dir * 10f32;
        let to = Vec3f::default();
        c.look_at(self.position, to, Vec3f::new(1f32, 1f32, 0f32).normalize());
        DirectLight {
            color: self.color,
            camera: [c],
            shadow: self.shadow,
            attenuation: self.attenuation,
            intensity: self.intensity,
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
    attenuation: Attenuation,
    intensity: f32,
}

impl PointLightBuilder {
    pub fn new() -> Self {
        Self {
            color: Color::new(1f32, 1f32, 1f32, 1f32),
            position: Vec3f::zeros(),
            shadow: ShadowConfig::default(),
            attenuation: Attenuation::default(),
            intensity: 1f32,
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
    pub fn attenuation(mut self, attenuation: Attenuation) -> Self {
        self.attenuation = attenuation;
        self
    }

    pub fn intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity;
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
            camera: [
                c.clone(),
                c.clone(),
                c.clone(),
                c.clone(),
                c.clone(),
                c.clone(),
            ],
            shadow: self.shadow,
            attenuation: self.attenuation,
            intensity: self.intensity,
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
    attenuation: Attenuation,
    intensity: f32,
}

impl SpotLightBuilder {
    pub fn new() -> Self {
        Self {
            color: Color::new(1f32, 1f32, 1f32, 1f32),
            position: Vec3f::zeros(),
            dir: Vec3f::new(0f32, -1f32, 0f32),
            cutoff: angle2rad(60f32),
            cutoff_outer: angle2rad(90f32),
            shadow: ShadowConfig::default(),
            attenuation: Attenuation::default(),
            intensity: 1f32,
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

    pub fn attenuation(mut self, attenuation: Attenuation) -> Self {
        self.attenuation = attenuation;
        self
    }
    pub fn intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity;
        self
    }

    pub fn build(self) -> SpotLight {
        let c = Camera::new();
        c.make_perspective(1.0f32, angle2rad(90f32), 0.1f32, 40f32);
        let to = self.position + self.dir * 100f32;
        c.look_at(self.position, to, Vec3f::new(0f32, 1f32, 1f32).normalize());
        SpotLight {
            color: self.color,
            pos: self.position,
            dir: self.dir,
            cutoff: self.cutoff,
            cutoff_outer: self.cutoff_outer,
            camera: [c],
            shadow: self.shadow,
            attenuation: self.attenuation,
            intensity: self.intensity,
        }
    }
}

impl Default for SpotLightBuilder {
    fn default() -> Self {
        Self::new()
    }
}
