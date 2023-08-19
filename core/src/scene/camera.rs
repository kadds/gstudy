use std::{fmt::Debug, sync::Mutex};

use crate::{
    types::{Frustum, Mat4x4f, Vec2f, Vec3f, Vec4f},
    util::angle2rad,
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
        Mat4x4f::new_perspective(self.aspect, self.fovy, self.near, self.far).into()
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
        Mat4x4f::new_orthographic(
            self.rect.x,
            self.rect.z,
            self.rect.w,
            self.rect.y,
            self.near,
            self.far,
        )
        .into()
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
        let inner = self.inner.lock().unwrap();
        match &inner.project_var {
            Project::Perspective(p) => {
                let deg_y = p.fovy.tan();
                let deg_x = deg_y * p.aspect;
                let world = inner.view;

                let f0 = world * Vec4f::new(-deg_x, -deg_y, 1f32, 1f32);
                let f1 = world * Vec4f::new(-deg_x, deg_y, 1f32, 1f32);
                let f2 = world * Vec4f::new(deg_x, -deg_y, 1f32, 1f32);
                let f3 = world * Vec4f::new(deg_x, deg_y, 1f32, 1f32);

                let pos = inner.from;

                Frustum::new([
                    pos + (p.near * f1).xyz(),
                    pos + (p.near * f3).xyz(),
                    pos + (p.near * f0).xyz(),
                    pos + (p.near * f2).xyz(),
                    pos + (p.far * f1).xyz(),
                    pos + (p.far * f3).xyz(),
                    pos + (p.far * f0).xyz(),
                    pos + (p.far * f2).xyz(),
                ])
            }
            Project::Orthographic(o) => {
                let world = inner.view;
                let pos = inner.from;
                let f0 = world * Vec4f::new(-o.rect.x, -o.rect.y, 1f32, 1f32);
                let f1 = world * Vec4f::new(-o.rect.x, o.rect.y, 1f32, 1f32);
                let f2 = world * Vec4f::new(o.rect.x, -o.rect.y, 1f32, 1f32);
                let f3 = world * Vec4f::new(o.rect.x, o.rect.y, 1f32, 1f32);
                Frustum::new([
                    pos + (o.near * f1).xyz(),
                    pos + (o.near * f3).xyz(),
                    pos + (o.near * f0).xyz(),
                    pos + (o.near * f2).xyz(),
                    pos + (o.far * f1).xyz(),
                    pos + (o.far * f3).xyz(),
                    pos + (o.far * f0).xyz(),
                    pos + (o.far * f2).xyz(),
                ])
            }
        }
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
            let up = inner.up.into();
            inner.view = Mat4x4f::look_at_rh(&from, &to, &up);
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
        if let Project::Orthographic(o) = &inner.project_var {
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
            Project::Perspective(p) => {
                return p.far;
            }
            Project::Orthographic(o) => {
                return o.far;
            }
        }
    }

    pub fn near(&self) -> f32 {
        let mut inner = self.inner.lock().unwrap();
        match &mut inner.project_var {
            Project::Perspective(p) => {
                return p.near;
            }
            Project::Orthographic(o) => {
                return o.near;
            }
        }
    }

    pub fn fovy(&self) -> f32 {
        let mut inner = self.inner.lock().unwrap();
        match &mut inner.project_var {
            Project::Perspective(p) => {
                return p.fovy;
            }
            Project::Orthographic(o) => {
                return 0f32;
            }
        }
    }

    pub fn aspect(&self) -> f32 {
        let mut inner = self.inner.lock().unwrap();
        match &mut inner.project_var {
            Project::Perspective(p) => {
                return p.aspect;
            }
            Project::Orthographic(o) => {
                return 0f32;
            }
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
