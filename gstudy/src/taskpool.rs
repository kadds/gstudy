use std::sync::{
    mpsc::{channel, Receiver, Sender},
    Arc, Condvar, Mutex,
};

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
