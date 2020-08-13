use std::{
    sync::{
        Arc,
        mpsc::{self, Sender, Receiver},
    },
    thread,
};
use type_map::concurrent::TypeMap;
use crate::{
    prelude::*,
    cfg::Config,
};


/// On frame of the "context stack".
#[derive(Debug)]
pub struct Frame {
    pub kind: FrameKind,

    /// Arbitrary data that can be set by operations.
    pub vars: TypeMap,
}

impl Frame {
    fn root() -> Self {
        Self {
            kind: FrameKind::Root,
            vars: TypeMap::new(),
        }
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
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
            top_frame: Arc::new(Frame::root()),
        }
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
        let frame = Frame {
            kind,
            vars: TypeMap::new(),
        };

        Self {
            config: self.config.clone(),
            top_frame: Arc::new(frame),
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
