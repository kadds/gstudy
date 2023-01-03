use std::{
    marker::PhantomData,
    sync::{
        atomic::AtomicU32,
        mpsc::{channel, Receiver, Sender},
        Arc, Condvar, Mutex,
    },
    thread::ScopedJoinHandle,
    time::Instant,
};

pub fn any_as_u8_slice<T: Sized>(p: &T) -> &[u8] {
    unsafe {
        ::std::slice::from_raw_parts((p as *const T) as *const u8, ::std::mem::size_of::<T>())
    }
}
pub fn any_as_u8_slice_array<T: Sized>(p: &[T]) -> &[u8] {
    unsafe {
        ::std::slice::from_raw_parts(
            (p.as_ptr() as *const T) as *const u8,
            ::std::mem::size_of::<T>() * p.len(),
        )
    }
}

pub fn any_as_x_slice_array<X: Sized, T: Sized>(p: &[T]) -> &[X] {
    unsafe {
        ::std::slice::from_raw_parts(
            (p.as_ptr() as *const T) as *const X,
            p.len() * ::std::mem::size_of::<T>() / ::std::mem::size_of::<X>(),
        )
    }
}

type WI = winit::window::CursorIcon;
type EI = egui::CursorIcon;

type WK = winit::event::VirtualKeyCode;
type EK = egui::Key;

pub fn match_winit_cursor(c: EI) -> Option<WI> {
    Some(match c {
        EI::Default => WI::Default,
        EI::None => return None,
        EI::ContextMenu => WI::ContextMenu,
        EI::Help => WI::Help,
        EI::PointingHand => WI::Hand,
        EI::Progress => WI::Progress,
        EI::Wait => WI::Wait,
        EI::Cell => WI::Cell,
        EI::Crosshair => WI::Crosshair,
        EI::Text => WI::Text,
        EI::VerticalText => WI::VerticalText,
        EI::Alias => WI::Alias,
        EI::Copy => WI::Copy,
        EI::Move => WI::Move,
        EI::NoDrop => WI::NoDrop,
        EI::NotAllowed => WI::NotAllowed,
        EI::Grab => WI::Grab,
        EI::Grabbing => WI::Grabbing,
        EI::AllScroll => WI::AllScroll,
        EI::ResizeHorizontal => WI::EwResize,
        EI::ResizeNeSw => WI::NeswResize,
        EI::ResizeNwSe => WI::NwseResize,
        EI::ResizeVertical => WI::NsResize,
        EI::ResizeEast => WI::EResize,
        EI::ResizeSouthEast => WI::SeResize,
        EI::ResizeSouth => WI::SResize,
        EI::ResizeSouthWest => WI::SwResize,
        EI::ResizeWest => WI::WResize,
        EI::ResizeNorthWest => WI::NwResize,
        EI::ResizeNorth => WI::NResize,
        EI::ResizeNorthEast => WI::NeResize,
        EI::ResizeColumn => WI::ColResize,
        EI::ResizeRow => WI::RowResize,

        EI::ZoomIn => WI::ZoomIn,
        EI::ZoomOut => WI::ZoomOut,
    })
}

pub fn match_egui_key(k: WK) -> Option<EK> {
    Some(match k {
        WK::Key1 => EK::Num1,
        WK::Key2 => EK::Num2,
        WK::Key3 => EK::Num3,
        WK::Key4 => EK::Num4,
        WK::Key5 => EK::Num5,
        WK::Key6 => EK::Num6,
        WK::Key7 => EK::Num7,
        WK::Key8 => EK::Num8,
        WK::Key9 => EK::Num9,
        WK::Key0 => EK::Num0,
        WK::A => EK::A,
        WK::B => EK::B,
        WK::C => EK::C,
        WK::D => EK::D,
        WK::E => EK::E,
        WK::F => EK::F,
        WK::G => EK::G,
        WK::H => EK::H,
        WK::I => EK::I,
        WK::J => EK::J,
        WK::K => EK::K,
        WK::L => EK::L,
        WK::M => EK::M,
        WK::N => EK::N,
        WK::O => EK::O,
        WK::P => EK::P,
        WK::Q => EK::Q,
        WK::R => EK::R,
        WK::S => EK::S,
        WK::T => EK::T,
        WK::U => EK::U,
        WK::V => EK::V,
        WK::W => EK::W,
        WK::X => EK::X,
        WK::Y => EK::Y,
        WK::Z => EK::Z,
        WK::Escape => EK::Escape,
        WK::Insert => EK::Insert,
        WK::Home => EK::Home,
        WK::Delete => EK::Delete,
        WK::Back => EK::Backspace,
        WK::Return => EK::Enter,
        WK::Space => EK::Space,
        WK::End => EK::End,
        WK::PageDown => EK::PageDown,
        WK::PageUp => EK::PageUp,
        WK::Left => EK::ArrowLeft,
        WK::Up => EK::ArrowUp,
        WK::Right => EK::ArrowRight,
        WK::Down => EK::ArrowDown,
        WK::Numpad0 => EK::Num0,
        WK::Numpad1 => EK::Num1,
        WK::Numpad2 => EK::Num2,
        WK::Numpad3 => EK::Num3,
        WK::Numpad4 => EK::Num4,
        WK::Numpad5 => EK::Num5,
        WK::Numpad6 => EK::Num6,
        WK::Numpad7 => EK::Num7,
        WK::Numpad8 => EK::Num8,
        WK::Numpad9 => EK::Num9,
        WK::Tab => EK::Tab,
        _ => {
            return None;
        }
    })
}

type Task = Box<dyn FnOnce() + Send>;

struct SharedData {
    rx: Mutex<Receiver<Task>>,
    stop: Mutex<(bool, u32)>,
    cv: Condvar,
}

impl SharedData {
    fn thread_main(&self) {
        let mut wait = false;
        loop {
            let f = {
                if wait {
                    let mut m = self.stop.lock().unwrap();
                    m = self.cv.wait(m).unwrap();
                }

                let rx = self.rx.lock().unwrap();
                match rx.try_recv() {
                    Ok(v) => v,
                    Err(_) => {
                        wait = true;
                        continue;
                    }
                }
            };
            wait = false;

            f();
        }
    }
}

struct TasksResult<E> {
    tasks: u32,
    result: Option<Result<(), E>>,
}

impl<E> Default for TasksResult<E> {
    fn default() -> Self {
        Self {
            tasks: 0,
            result: None,
        }
    }
}

struct TaskPoolShared<E> {
    tasks: Mutex<TasksResult<E>>,
    wait_cv: Condvar,
}

pub struct TaskPoolBatch<E> {
    inner: Arc<SharedData>,
    sender: Sender<Task>,
    ts: Arc<TaskPoolShared<E>>,
}

impl<E> TaskPoolBatch<E>
where
    E: Send + 'static,
{
    pub fn wait(self) -> Result<(), E> {
        let mut m = self.ts.tasks.lock().unwrap();
        while m.tasks != 0 {
            m = self.ts.wait_cv.wait(m).unwrap()
        }
        match m.result.take() {
            Some(v) => v,
            None => Ok(()),
        }
    }

    pub fn execute<'a, F: FnOnce() -> Result<(), E> + Send + 'a>(&'a self, f: F)
    where
        E: 'a,
    {
        {
            let mut m = self.ts.tasks.lock().unwrap();
            m.tasks += 1;
        }
        let ts = self.ts.clone();
        let mut box_f = Box::new(f);
        let f = box_f.as_mut() as *mut (dyn FnOnce() -> Result<(), E> + Send + 'a);
        let f: *mut (dyn FnOnce() -> Result<(), E> + Send + 'static) =
            unsafe { std::mem::transmute(f) };

        let new_box_f = unsafe { Box::from_raw(f) };
        Box::into_raw(box_f);

        self.sender
            .send(Box::new(move || {
                let ret = new_box_f();
                let mut m = ts.tasks.lock().unwrap();
                if let Err(e) = ret {
                    m.result = Some(Err(e));
                }
                m.tasks -= 1;
                if m.tasks == 0 {
                    ts.wait_cv.notify_all();
                }
            }))
            .unwrap();

        self.inner.cv.notify_one();
    }
}

pub struct TaskPool {
    inner: Arc<SharedData>,
    sender: Sender<Task>,
}

impl TaskPool {
    pub fn make_batch<E: Send>(&self) -> TaskPoolBatch<E> {
        TaskPoolBatch {
            inner: self.inner.clone(),
            sender: self.sender.clone(),
            ts: Arc::new(TaskPoolShared {
                tasks: Mutex::new(TasksResult::default()),
                wait_cv: Condvar::new(),
            }),
        }
    }
}

pub struct TaskPoolBuilder {
    n: usize,
    name: String,
}

impl TaskPoolBuilder {
    pub fn new() -> Self {
        Self {
            n: num_cpus::get(),
            name: "".to_owned(),
        }
    }

    pub fn nums(mut self, n: usize) -> Self {
        self.n = n;
        self
    }

    pub fn name<S: Into<String>>(mut self, name: S) -> Self {
        self.name = name.into();
        self
    }

    pub fn build(self) -> TaskPool {
        let (tx, rx) = channel();
        let pool = TaskPool {
            inner: Arc::new(SharedData {
                rx: Mutex::new(rx),
                stop: Mutex::new((false, self.n as u32)),
                cv: Condvar::new(),
            }),
            sender: tx,
        };

        for _ in 0..self.n {
            let sd = pool.inner.clone();
            std::thread::Builder::new()
                .name(self.name.clone())
                .spawn(move || sd.thread_main())
                .unwrap();
        }

        pool
    }
}
