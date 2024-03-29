use std::{fmt::Debug, io::Write, sync::Mutex};

use crate::{
    types::{Frustum, Mat4x4f, Vec2f, Vec3f, Vec4f},
    util::{angle2rad, any_as_u8_slice},
};

#[derive(Debug, Clone)]
struct PerspectiveProject {
    aspect: f32,
    fovy: f32,
    near: f32,
    far: f32,
}

impl Default for PerspectiveProject {
    fn default() -> Self {
        Self {
            aspect: 1f32,
            fovy: angle2rad(90f32),
            near: 0.1f32,
            far: 100f32,
        }
    }
}

impl PerspectiveProject {
    pub fn gen(&self) -> Mat4x4f {
        let mut res = Mat4x4f::new_perspective(self.aspect, self.fovy, self.near, self.far);
        res.append_nonuniform_scaling_mut(&Vec3f::new(1f32, 1f32, 0.5f32));
        res.append_translation_mut(&Vec3f::new(0f32, 0f32, 0.5f32));
        res
    }
}

#[derive(Debug, Clone)]
struct OrthographicProject {
    rect: Vec4f,
    near: f32,
    far: f32,
}

impl Default for OrthographicProject {
    fn default() -> Self {
        Self {
            rect: Vec4f::new(0f32, 0f32, 1f32, 1f32),
            near: 0.1f32,
            far: 10f32,
        }
    }
}

impl OrthographicProject {
    pub fn gen(&self) -> Mat4x4f {
        let mut res = Mat4x4f::new_orthographic(
            self.rect.x,
            self.rect.z,
            self.rect.y,
            self.rect.w,
            self.near,
            self.far,
        );
        res.append_nonuniform_scaling_mut(&Vec3f::new(1f32, 1f32, 0.5f32));
        res.append_translation_mut(&Vec3f::new(0f32, 0f32, 0.5f32));
        res
    }
}

#[derive(Debug, Clone)]
enum Project {
    Perspective(PerspectiveProject),
    Orthographic(OrthographicProject),
}

impl Default for Project {
    fn default() -> Self {
        Self::Perspective(PerspectiveProject::default())
    }
}

impl Project {
    pub fn gen(&self) -> Mat4x4f {
        match self {
            Project::Perspective(p) => p.gen(),
            Project::Orthographic(o) => o.gen(),
        }
    }
}

#[repr(C)]
struct Uniform3d {
    mat: Mat4x4f,
    direction: Vec4f,
}

#[repr(C)]
struct Uniform2d {
    size: Vec4f,
}

#[derive(Debug, Clone)]
struct Inner {
    project_var: Project,
    projection: Mat4x4f,

    from: Vec3f,
    to: Vec3f,
    up: Vec3f,
    view: Mat4x4f,

    dirty_project: bool,
    dirty_view: bool,
}

#[derive(Debug)]
pub struct Camera {
    inner: Mutex<Inner>,
}

impl Clone for Camera {
    fn clone(&self) -> Self {
        Self {
            inner: Mutex::new(self.inner.lock().unwrap().clone()),
        }
    }
}

impl Camera {
    pub fn new() -> Self {
        Self {
            inner: Inner {
                projection: Mat4x4f::identity(),
                view: Mat4x4f::identity(),
                from: Vec3f::new(1f32, 1f32, 1f32),
                to: Vec3f::new(0f32, 0f32, 0f32),
                up: Vec3f::new(0f32, 1f32, 0f32),

                project_var: Project::default(),
                dirty_project: true,
                dirty_view: true,
            }
            .into(),
        }
    }

    pub fn copy_from(&self, camera: &Camera) {
        let mut s = self.inner.lock().unwrap();
        let u = camera.inner.lock().unwrap();
        *s = u.clone();
        s.dirty_project = true;
        s.dirty_view = true;
    }

    pub fn frustum_worldspace(&self) -> Frustum {
        let vp = self.vp();
        let rev = vp.try_inverse().unwrap();
        let pos = self.from();
        let to = self.to();
        let up = self.up();

        let nlt = Vec4f::new(-1f32, 1f32, 0f32, 1f32);
        let nlb = Vec4f::new(-1f32, -1f32, 0f32, 1f32);
        let nrt = Vec4f::new(1f32, 1f32, 0f32, 1f32);
        let nrb = Vec4f::new(1f32, -1f32, 0f32, 1f32);

        let flt = Vec4f::new(-1f32, 1f32, 1f32, 1f32);
        let flb = Vec4f::new(-1f32, -1f32, 1f32, 1f32);
        let frt = Vec4f::new(1f32, 1f32, 1f32, 1f32);
        let frb = Vec4f::new(1f32, -1f32, 1f32, 1f32);

        let nlt = rev * nlt;
        let nrt = rev * nrt;
        let nlb = rev * nlb;
        let nrb = rev * nrb;

        let flt = rev * flt;
        let frt = rev * frt;
        let flb = rev * flb;
        let frb = rev * frb;

        Frustum::new(
            &[
                nlt.xyz() / nlt.w,
                nrt.xyz() / nrt.w,
                nlb.xyz() / nlb.w,
                nrb.xyz() / nrb.w,
                flt.xyz() / flt.w,
                frt.xyz() / frt.w,
                flb.xyz() / flb.w,
                frb.xyz() / frb.w,
            ],
            pos,
            to,
            up,
        )
    }

    pub fn uniform_3d(&self) -> Vec<u8> {
        let mut data = vec![];
        let vp = self.vp();
        let dir = (self.to() - self.from()).normalize();
        let uniform = Uniform3d {
            mat: vp,
            direction: Vec4f::new(dir.x, dir.y, dir.z, 0.0f32),
        };
        let _ = data.write_all(any_as_u8_slice(&uniform));
        data
    }

    pub fn uniform_shadow_3d(&self) -> Vec<u8> {
        let mut data = vec![];
        let vp = self.vp();
        let dir = (self.to() - self.from()).normalize();
        let uniform = Uniform3d {
            mat: vp,
            direction: Vec4f::new(dir.x, dir.y, dir.z, 0.0f32),
        };
        let _ = data.write_all(any_as_u8_slice(&uniform));
        let _ = data.write_all(any_as_u8_slice(&self.near()));
        let _ = data.write_all(any_as_u8_slice(&self.far()));
        if self.is_perspective() {
            let _ = data.write_all(any_as_u8_slice(&1.0f32));
        } else {
            let _ = data.write_all(any_as_u8_slice(&-1.0f32));
        }
        let _ = data.write_all(any_as_u8_slice(&0.0f32));
        data
    }

    pub fn vp(&self) -> Mat4x4f {
        let mut inner = self.inner.lock().unwrap();
        if inner.dirty_project {
            inner.projection = inner.project_var.gen();
            inner.dirty_project = false;
        }
        if inner.dirty_view {
            let from = inner.from.into();
            let to = inner.to.into();
            let up = &inner.up;
            inner.view = Mat4x4f::look_at_rh(&from, &to, up);
            inner.dirty_view = false;
        }
        inner.projection * inner.view
    }

    pub fn make_orthographic(&self, rect: Vec4f, near: f32, far: f32) {
        let mut inner = self.inner.lock().unwrap();
        inner.dirty_project = true;
        inner.project_var = Project::Orthographic(OrthographicProject { rect, near, far });
    }

    pub fn make_perspective(&self, aspect: f32, fovy: f32, near: f32, far: f32) {
        let mut inner = self.inner.lock().unwrap();
        inner.dirty_project = true;
        inner.project_var = Project::Perspective(PerspectiveProject {
            fovy,
            aspect,
            near,
            far,
        });
    }

    pub fn set_aspect(&self, aspect: f32) -> bool {
        let mut inner = self.inner.lock().unwrap();
        inner.dirty_project = true;
        if let Project::Perspective(project) = &mut inner.project_var {
            project.aspect = aspect;
            return true;
        }
        false
    }

    pub fn set_fov(&self, fov: f32) -> bool {
        let mut inner = self.inner.lock().unwrap();
        if let Project::Perspective(project) = &mut inner.project_var {
            project.fovy = fov;
            inner.dirty_project = true;
            return true;
        }
        false
    }

    pub fn set_near(&self, near: f32) {
        let mut inner = self.inner.lock().unwrap();
        inner.dirty_project = true;
        match &mut inner.project_var {
            Project::Perspective(p) => {
                p.near = near;
            }
            Project::Orthographic(o) => {
                o.near = near;
            }
        }
    }

    pub fn set_far(&self, far: f32) {
        let mut inner = self.inner.lock().unwrap();
        inner.dirty_project = true;
        match &mut inner.project_var {
            Project::Perspective(p) => {
                p.far = far;
            }
            Project::Orthographic(o) => {
                o.far = far;
            }
        }
    }

    pub fn is_perspective(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        if let Project::Orthographic(_o) = &inner.project_var {
            false
        } else {
            true
        }
    }

    pub fn look_at(&self, from: Vec3f, to: Vec3f, up: Vec3f) {
        let mut inner = self.inner.lock().unwrap();
        inner.from = from;
        inner.to = to;
        inner.up = up;
        inner.dirty_view = true;
    }
    pub fn from(&self) -> Vec3f {
        let inner = self.inner.lock().unwrap();
        inner.from
    }
    pub fn to(&self) -> Vec3f {
        let inner = self.inner.lock().unwrap();
        inner.to
    }
    pub fn up(&self) -> Vec3f {
        let inner = self.inner.lock().unwrap();
        inner.up
    }
    pub fn right(&self) -> Vec3f {
        let inner = self.inner.lock().unwrap();
        (inner.from - inner.to).cross(&inner.up)
    }
    pub fn far(&self) -> f32 {
        let mut inner = self.inner.lock().unwrap();
        match &mut inner.project_var {
            Project::Perspective(p) => p.far,
            Project::Orthographic(o) => o.far,
        }
    }

    pub fn near(&self) -> f32 {
        let mut inner = self.inner.lock().unwrap();
        match &mut inner.project_var {
            Project::Perspective(p) => p.near,
            Project::Orthographic(o) => o.near,
        }
    }

    pub fn fovy(&self) -> f32 {
        let mut inner = self.inner.lock().unwrap();
        match &mut inner.project_var {
            Project::Perspective(p) => p.fovy,
            Project::Orthographic(_o) => 0f32,
        }
    }

    pub fn aspect(&self) -> f32 {
        let mut inner = self.inner.lock().unwrap();
        match &mut inner.project_var {
            Project::Perspective(p) => p.aspect,
            Project::Orthographic(_o) => 0f32,
        }
    }

    pub fn width_height(&self) -> Vec2f {
        let inner = self.inner.lock().unwrap();
        if let Project::Orthographic(o) = &inner.project_var {
            Vec2f::new(o.rect.z - o.rect.x, o.rect.w - o.rect.y)
        } else {
            Vec2f::zeros()
        }
    }
}
