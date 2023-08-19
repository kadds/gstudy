use core::{
    context::{RContext, RContextRef},
    event::{EventProcessor, EventSender, EventSource},
    scene::Scene,
};
use std::{
    any::{Any, TypeId},
    cell::{Ref, RefCell},
    rc::Rc,
    sync::{Arc, Mutex},
};

use container::Container;
use plugin::{Plugin, PluginFactory};

use crate::plugin::{CoreFactoryList, Runner};

pub mod container;
pub mod plugin;

pub struct AppEventContext<'a> {
    pub source: &'a dyn EventSource,
    pub container: &'a Container,
}

pub trait AppEventProcessor {
    fn on_event(&mut self, context: &AppEventContext, event: &dyn Any);
}

pub struct App {
    context: RContextRef,
    container: Arc<Container>,
    plugin_factory_list: Vec<Box<dyn PluginFactory>>,
    processors: Rc<RefCell<Vec<Box<dyn AppEventProcessor>>>>,
}

impl App {
    pub fn new(context: RContextRef) -> Self {
        Self {
            context,
            container: Arc::new(Container::default()),
            plugin_factory_list: vec![],
            processors: Rc::new(RefCell::new(vec![])),
        }
    }

    pub fn register_plugin<P: PluginFactory + 'static>(&mut self, plugin: P) {
        self.plugin_factory_list.push(Box::new(plugin));
    }

    pub fn container(&self) -> &Container {
        &self.container
    }

    pub fn add_event_processor(&mut self, ep: Box<dyn AppEventProcessor>) {
        self.processors.borrow_mut().push(ep);
    }

    pub fn run(&self) {
        self.container.register_arc(self.context.clone());
        self.container.register(Scene::new(self.context.clone()));

        let mut plugins = vec![];
        let mut looper = None;
        for p in &self.plugin_factory_list {
            let info = p.info();
            let ins = p.create(&self.container);
            if info.has_looper {
                looper = Some(p.create_looper(&self.container).unwrap());
            }

            plugins.push(ins);
            log::info!(
                "plugin \"{}\" load verison \"{}\" done",
                info.name,
                info.version
            );
        }

        // install factories

        let mut factory_list = CoreFactoryList::default();

        for p in &mut plugins {
            let core_factory_list = p.load_factory();
            factory_list.materials.extend(core_factory_list.materials);
        }

        for p in &mut plugins {
            p.install_factory(&mut factory_list);
        }

        // run looper
        struct ARunner {
            processors: Rc<RefCell<Vec<Box<dyn AppEventProcessor>>>>,
            plugins: Vec<Box<dyn Plugin>>,
            container: Arc<Container>,
        }

        impl Runner for ARunner {
            fn startup(&self, proxy: &dyn EventSender) {
                proxy.send_event(Box::new(Event::Startup));
            }
        }

        impl EventProcessor for ARunner {
            fn on_event(
                &mut self,
                source: &dyn core::event::EventSource,
                event: &dyn Any,
            ) -> core::event::ProcessEventResult {
                let context = &AppEventContext {
                    source: source,
                    container: &self.container,
                };

                for p in self.plugins.iter_mut() {
                    p.on_event(context, event);
                }
                for p in self.processors.borrow_mut().iter_mut() {
                    p.on_event(context, event);
                }

                core::event::ProcessEventResult::Received
            }
        }

        looper.unwrap().run(
            &self.container,
            Rc::new(RefCell::new(ARunner {
                processors: self.processors.clone(),
                plugins,
                container: self.container.clone(),
            })),
        );
    }
}

#[derive(Debug)]
pub enum Event {
    Startup,
}
