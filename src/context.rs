use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, RwLock,
        mpsc::{self, Sender, Receiver},
    },
    thread,
};
use type_map::concurrent::TypeMap;
use crate::{
    prelude::*,
    cfg::Config,
    op::WorkDir,
};


/// On frame of the "context stack".
#[derive(Debug)]
pub struct Frame {
    pub kind: FrameKind,

    /// Arbitrary data that can be set by operations.
    vars: RwLock<TypeMap>,
}

impl Frame {
    fn new(kind: FrameKind) -> Self {
        Self {
            kind,
            vars: RwLock::new(TypeMap::new()),
        }
    }

    fn root() -> Self {
        Self::new(FrameKind::Root)
    }

    pub fn insert_var<T: Send + Sync + 'static>(&self, val: T) -> Option<T> {
        self.vars.write()
            .expect("var type map poisoned :(")
            .insert(val)
    }

    pub fn get_var<T: Clone + 'static>(&self) -> Option<T> {
        self.vars.read()
            .expect("var type map poisoned :(")
            .get()
            .cloned()
    }
}

/// The kind of context frame.
#[derive(Debug)]
pub enum FrameKind {
    Root,
    Task {
        name: String,
        parent: Arc<Frame>,
    },
    Operation {
        name: String,
        parent: Arc<Frame>,
    },
}

/// Contains global information and information about the "execution context".
/// The latter is mostly just a stack describing what tasks and operations lead
/// to this current "execution".
#[derive(Debug, Clone)]
pub struct Context {
    pub config: Arc<Config>,
    pub top_frame: Arc<Frame>,
}

impl Context {
    /// Creates a new context.
    pub fn new(config: Config, config_file: Option<&Path>) -> Result<Self> {
        let mut path = config_file.unwrap_or(Path::new(cfg::DEFAULT_FILENAME)).to_path_buf();
        if path.is_relative() {
            path = std::env::current_dir()?.join(path);
        }
        let root_path = path.parent().expect("path to config file has not final component");

        let root_frame = Frame::root();
        root_frame.insert_var(WorkDir(root_path.into()));

        Ok(Self {
            config: Arc::new(config),
            top_frame: Arc::new(root_frame),
        })
    }

    /// Returns an iterator over all frames of the execution context.
    pub fn frames(&self) -> impl Iterator<Item = &Frame> {
        std::iter::successors(Some(&*self.top_frame), |frame| {
            match &frame.kind {
                FrameKind::Root => None,
                FrameKind::Task { parent, .. } => Some(&**parent),
                FrameKind::Operation { parent, .. } => Some(&**parent),
            }
        })
    }

    pub fn root_frame(&self) -> &Frame {
        self.frames().last().unwrap()
    }

    pub fn get_closest_var<T: Clone + 'static>(&self) -> Option<T> {
        self.frames().find_map(|f| f.get_var())
    }

    pub fn workdir(&self) -> PathBuf {
        self.get_closest_var::<WorkDir>().expect("bug: no root workdir").0
    }

    /// Joins the `new_path` with the current workdir context. Three possible
    /// cases:
    /// - `new_path` is absolute: `new_path` is returned
    /// - `new_path` starts with `./`: the closest `WorkDir` variable joined
    ///   with `new_path` is returned.
    /// - Else: the path of the config file (minus file name) joined with
    ///   `new_path` is returned.
    pub fn join_workdir(&self, new_path: impl AsRef<Path>) -> PathBuf {
        let new_path = new_path.as_ref();
        match () {
            () if new_path.is_absolute() => new_path.to_path_buf(),
            () if new_path.starts_with(".") => {
                self.workdir().join(new_path.strip_prefix(".").unwrap())
            }
            _ => {
                let base = self.root_frame().get_var::<WorkDir>().expect("bug: no root workdir");
                base.0.join(new_path)
            }
        }
    }

    /// Creates a new context that has a `Task` frame with the given name added
    /// on top.
    pub fn fork_task(&self, name: impl Into<String>) -> Self {
        self.fork(FrameKind::Task {
            name: name.into(),
            parent: self.top_frame.clone(),
        })
    }

    /// Creates a new context that has a `Operation` frame with the given name
    /// added on top.
    pub fn fork_op(&self, name: impl Into<String>) -> Self {
        self.fork(FrameKind::Operation {
            name: name.into(),
            parent: self.top_frame.clone(),
        })
    }

    fn fork(&self, kind: FrameKind) -> Self {
        Self {
            config: self.config.clone(),
            top_frame: Arc::new(Frame::new(kind)),
        }
    }

    /// Returns the label used for terminal messages. Usually just the name of
    /// the closest `task`.
    pub fn frame_label(&self) -> String {
        match &self.top_frame.kind {
            FrameKind::Root => "".into(),
            FrameKind::Task { name, .. } => name.clone(),
            FrameKind::Operation { name, .. } => {
                let mut out = name.clone();
                for frame in self.frames().skip(1) {
                    match &frame.kind {
                        FrameKind::Root => panic!("bug: operation frame is child of root frame"),
                        FrameKind::Task { name, .. } => return format!("{}.{}", name, out),
                        FrameKind::Operation { name, .. } => out = format!("{}.{}", name, out),
                    }
                }

                panic!("bug: operation frame is root frame");
            }
        }
    }
}
