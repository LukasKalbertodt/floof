use std::io::Write;
use once_cell::sync::OnceCell;
use termcolor::{Buffer, BufferWriter, ColorChoice};
use crate::{
    Args,
    prelude::*,
};


/// Level of verbosity, controlling which messages to print and which not.
#[derive(Clone, Copy, PartialEq)]
pub enum Verbosity {
    Normal,
    Verbose,
    Trace,
}

impl Verbosity {
    fn from_count(count: u8) -> Result<Self> {
        match count {
            0 => Ok(Self::Normal),
            1 => Ok(Self::Verbose),
            2 => Ok(Self::Trace),
            _ => Err(anyhow!("invalid verbosity level: flag -v specified too often")),
        }
    }
}

// We store these two global so that easy macros can be used. We don't really
// have any downside from making them global.
pub static WRITER: OnceCell<BufferWriter> = OnceCell::new();
pub static VERBOSITY: OnceCell<Verbosity> = OnceCell::new();

pub const PREFIX: &str = "════════════╡";

/// Initializes the UI system with the settings from the args.
pub fn init(args: &Args) -> Result<()> {
    WRITER.set(BufferWriter::stdout(args.color))
        .map_err(|_| ())
        .expect("bug: ui already initialized");
    VERBOSITY.set(Verbosity::from_count(args.verbose)?)
        .map_err(|_| ())
        .expect("bug: ui already initialized");

    Ok(())
}

pub fn print_prefix(
    mut buf: &mut Buffer,
    icon: &str,
    task: Option<&str>,
    op: Option<&str>,
) -> Result<(), std::io::Error> {
    bunt::write!(buf, "{[blue]} {}  ", crate::ui::PREFIX, icon)?;

    if let Some(task) = task {
        bunt::write!(buf, "{$black+intense}[{[blue+intense+bold]}]{/$}", task)?;
    }
    if let Some(op) = op {
        bunt::write!(buf, "{$black+intense}[{[blue+intense]}]{/$}", op)?;
    }

    if task.is_some() || op.is_some() {
        write!(buf, " ")?;
    }

    Ok(())
}

/// Emit a message.
macro_rules! msg {
    (@to_option -) => { None };
    (@to_option [$inner:expr]) => { Some($inner) };

    // Still unused: 📸 🔔 💧 ⚡ ❄ 🌊 🌈 🌀 ⏳ ⌛ 💡 👂 👋
    (@icon -) => { " " };
    (@icon info) => { "ℹ️" };
    (@icon warn) => { "⚠️" };
    (@icon fire) => { "🔥" };
    (@icon run) => { "▶️" };
    (@icon reload) => { "♻️" };
    (@icon eye) => { "👁" };
    (@icon $other:tt) => { $other };

    ($icon:tt $task:tt $op:tt $($t:tt)*) => {{
        let w = crate::ui::WRITER.get().expect("bug: ui not initialized yet");
        let mut buf = w.buffer();
        (|| -> Result<(), std::io::Error> {
            crate::ui::print_prefix(
                &mut buf,
                msg!(@icon $icon),
                msg!(@to_option $task),
                msg!(@to_option $op),
            )?;
            bunt::writeln!(buf, $($t)*)?;

            w.print(&buf)?;
            Ok(())
        })().expect("error writing to stdout :-(");
    }};
}

/// Emit a verbose message that is only printed if the verbosity level is
/// `Verbose` or `Trace`.
macro_rules! verbose {
    ($($t:tt)*) => {{
        let level = *crate::ui::VERBOSITY.get().expect("bug: ui not initialized yet");
        if level == crate::ui::Verbosity::Verbose || level == crate::ui::Verbosity::Trace {
            msg!($($t)*);
        }
    }};
}


// impl Ui {
//     // pub fn watching(&self, task: &str, paths: &[impl fmt::Display]) {
//     //     let mut paths_str = String::new();
//     //     const MAX_PATHS: usize = 3;
//     //     for (i, path) in paths.iter().enumerate() {
//     //         match i {
//     //             0 => paths_str = format!("`{}`", path),
//     //             i if i >= MAX_PATHS => {
//     //                 paths_str += &format!(" and {} more", paths.len() - MAX_PATHS);
//     //                 break;
//     //             }
//     //             i if i == paths.len() - 1 => paths_str +=  &format!(" and `{}`", path),
//     //             _ => paths_str += &format!(", `{}`", path),
//     //         }
//     //     }

//     //     Message::service("👁 ", format!("[{}] watching: {}", task, paths_str)).emit(self);
//     // }

//     // pub fn listening(&self, addr: &SocketAddr) {
//     //     Message::service("🌀", format!("listening on 'http://{}'", addr)).emit(self);
//     // }

//     // pub fn listening_ws(&self, addr: &SocketAddr) {
//     //     Message::service("🌀", format!("websockets listening on 'ws://{}'", addr)).emit(self);
//     // }

//     // pub fn exiting_no_watcher(&self) {
//     //     Message::status("", "no HTTP server or watcher configured: we are done already! Bye :)")
//     //         .emit(self);
//     // }

//     // pub fn change_detected(&self, task: &str, debounce_duration: Duration) {
//     //     // 📸 🔔 🔥 💧 ⚡ ❄ 🌊 🌈 🌀 ⏳ ⌛ 💡 👂

//     //     let duration = if debounce_duration >= Duration::from_secs(1) {
//     //         format!("{:.1?}", debounce_duration)
//     //     } else {
//     //         format!("{:.0?}", debounce_duration)
//     //     };
//     //     let msg = format!(
//     //         "[{}] change detected, debouncing for {}...",
//     //         task,
//     //         duration,
//     //     );
//     //     Message::status("⏳", msg)
//     //         .without_line_ending()
//     //         .emit(self);
//     // }

//     // pub fn run_on_change_handlers(&self, task: &str) {
//     //     let msg = format!("[{}] change detected, executing handler...", task);
//     //     Message::status("🔥", msg)
//     //         .replace_previous()
//     //         .emit(self);
//     // }

//     // pub fn run_command(&self, handler: &str, command: &Command) {
//     //     let msg = format!("running ({}): {}", handler, command);
//     //     Message::status("▶️ ", msg).emit(self);
//     // }

//     // pub fn reload_browser(&self, task: &str) {
//     //     let msg = format!("[{}] reloading browser", task);
//     //     Message::status("♻️ ", msg)
//     //         .replace_previous()
//     //         .emit(self);
//     // }

//     // pub fn port_wait_timeout(&self, target: SocketAddr, duration: Duration) {
//     //     let msg = format!(
//     //         "Timeout reached when listening for proxy target ({}) to get ready. \
//     //             Port refused connection for {:?}. Autoreload is cancelled.",
//     //         target,
//     //         duration
//     //     );
//     //     Message::error("⚠", msg).emit(self);
//     // }
// }
