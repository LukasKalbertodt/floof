use std::{
    sync::{
        Arc,
        mpsc::{self, Sender, Receiver},
    },
    thread,
};
use crate::{
    prelude::*,
    cfg::Config,
};


/// On frame of the "context stack".
#[derive(Debug)]
pub struct Frame {
    // TODO: maybe make `Frame` an enum and "inline" `FrameKind`
    parent: Option<Arc<Frame>>,
    kind: FrameKind,
}

impl Frame {
    fn root() -> Self {
        Self {
            parent: None,
            kind: FrameKind::Root,
        }
    }
}

/// The kind of context frame.
#[derive(Debug)]
pub enum FrameKind {
    /// Exists only once and does not have a parent.
    Root,
    Task(String),
    Operation(String),
}

/// Contains global information and information about the "execution context".
/// The latter is mostly just a stack describing what tasks and operations lead
/// to this current "execution".
#[derive(Debug, Clone)]
pub struct Context {
    pub config: Arc<Config>,
    pub frames: Arc<Frame>,
}

impl Context {
    /// Creates a new context.
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
            frames: Arc::new(Frame::root()),
        }
    }

    /// Returns an iterator over all frames of the execution context.
    pub fn frames(&self) -> impl Iterator<Item = &Frame> {
        std::iter::successors(Some(&*self.frames), |frame| frame.parent.as_ref().map(|p| &**p))
    }

    /// Creates a new context that has the given frame added.
    pub fn fork(&self, kind: FrameKind) -> Self {
        let frame = Arc::new(Frame {
            parent: Some(self.frames.clone()),
            kind,
        });

        Self {
            config: self.config.clone(),
            frames: frame,
        }
    }

    /// Returns the label used for terminal messages. Usually just the name of
    /// the closest `task`.
    pub fn frame_label(&self) -> String {
        match &self.frames.kind {
            FrameKind::Root => "".into(),
            FrameKind::Task(name) => name.clone(),
            FrameKind::Operation(name) => {
                let mut out = name.clone();
                for frame in self.frames().skip(1) {
                    match &frame.kind {
                        FrameKind::Root => panic!("bug: operation frame is child of root frame"),
                        FrameKind::Task(name) => return format!("{}.{}", name, out),
                        FrameKind::Operation(name) => out = format!("{}.{}", name, out),
                    }
                }

                panic!("bug: operation frame is root frame");
            }
        }
    }
}
