use std::{
    io::{self, Write},
    sync::{atomic::{Ordering, AtomicBool}, mpsc::Sender, Arc}, fmt, net::SocketAddr, time::Duration,
};
use anyhow::Error;
use termcolor::{BufferWriter, ColorChoice, ColorSpec, WriteColor};
use crate::config::Command;


#[derive(Clone)]
pub struct Ui {
    writer: Arc<BufferWriter>,
    errors: Sender<Error>,
    at_start_of_line: Arc<AtomicBool>,
}

const PREFIX: &str = "════════════";

impl Ui {
    pub fn new(errors: Sender<Error>) -> Self {
        Self {
            // TODO: maybe make color choice configurable
            writer: Arc::new(BufferWriter::stdout(ColorChoice::Auto)),
            errors,
            at_start_of_line: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn watching(&self, action: &str, paths: &[impl fmt::Display]) {
        let mut paths_str = String::new();
        const MAX_PATHS: usize = 3;
        for (i, path) in paths.iter().enumerate() {
            match i {
                0 => paths_str = format!("`{}`", path),
                i if i >= MAX_PATHS => {
                    paths_str += &format!(" and {} more", paths.len() - MAX_PATHS);
                    break;
                }
                i if i == paths.len() - 1 => paths_str +=  &format!(" and `{}`", path),
                _ => paths_str += &format!(", `{}`", path),
            }
        }

        Message::service("👁 ", format!("watching for '{}': {}", action, paths_str)).emit(self);
    }

    pub fn listening(&self, addr: &SocketAddr) {
        Message::service("🌀", format!("listening on 'http://{}'", addr)).emit(self);
    }

    pub fn listening_ws(&self, addr: &SocketAddr) {
        Message::service("🌀", format!("websockets listening on 'ws://{}'", addr)).emit(self);
    }

    pub fn exiting_no_watcher(&self) {
        Message::status("👋", "no HTTP server or watcher configured: we are done already! Bye :)")
            .emit(self);
    }

    pub fn change_detected(&self, action: &str, debounce_duration: Duration) {
        // 📸 🔔 🔥 💧 ⚡ ❄ 🌊 🌈 🌀 ⏳ ⌛ 💡 👂

        let duration = if debounce_duration >= Duration::from_secs(1) {
            format!("{:.1?}", debounce_duration)
        } else {
            format!("{:.0?}", debounce_duration)
        };
        let msg = format!(
            "change detected for action '{}', debouncing for {}...",
            action,
            duration,
        );
        Message::status("⏳", msg)
            .without_line_ending()
            .emit(self);
    }

    pub fn run_on_change_handlers(&self, action: &str) {
        let msg = format!("change detected for action '{}', executing handler...", action);
        Message::status("🔥", msg)
            .replace_previous()
            .emit(self);
    }

    pub fn run_command(&self, handler: &str, command: &Command) {
        let msg = format!("running ({}): {}", handler, command);
        Message::status("🐇", msg).emit(self);
    }
}


// ╞════════════╡
// ════════════╡ Peter
// ━━━┝ ┣ ┫  ┥
// ━━━━━━━━━━━━┥ Peter

struct Message {
    kind: MessageKind,
    icon: String,
    msg: String,
    end_line: bool,
    replace_previous: bool,
}

impl Message {
    fn new(kind: MessageKind, icon: impl Into<String>, msg: impl Into<String>) -> Self {
        Self {
            kind,
            icon: icon.into(),
            msg: msg.into(),
            end_line: true,
            replace_previous: false,
        }
    }

    fn service(icon: impl Into<String>, msg: impl Into<String>) -> Self {
        Self::new(MessageKind::Service, icon, msg)
    }

    fn status(icon: impl Into<String>, msg: impl Into<String>) -> Self {
        Self::new(MessageKind::Status, icon, msg)
    }

    fn without_line_ending(self) -> Self {
        Self {
            end_line: false,
            ..self
        }
    }

    fn replace_previous(self) -> Self {
        Self {
            replace_previous: true,
            ..self
        }
    }

    fn emit(&self, ui: &Ui) {
        let run = || -> Result<(), Error> {
            let mut buf = ui.writer.buffer();

            if !self.replace_previous && !ui.at_start_of_line.load(Ordering::SeqCst) {
                writeln!(buf)?;
            } else if self.replace_previous {
                let spaces = "                    ";
                write!(buf, "\r{0}{0}{0}{0}{0}{0}\r", spaces)?;
            }

            buf.set_color(&colors::prefix())?;
            write!(buf, "{}", PREFIX)?;
            buf.set_color(&colors::icon())?;
            write!(buf, " {} ", self.icon)?;
            buf.set_color(&self.kind.msg_color())?;
            write!(buf, "{}", self.msg)?;

            if self.end_line {
                writeln!(buf)?;
            }

            buf.reset()?;
            ui.writer.print(&buf)?;
            io::stdout().flush()?;

            Ok(())
        };

        if let Err(e) = run() {
            // We ignore the error: if the channel is dropped, all threads will
            // soon be removed anyway.
            let _ = ui.errors.send(e);
        }
    }
}

enum MessageKind {
    /// A message about a service, usually about starting one. E.g. starting the
    /// HTTP server or starting to listen.
    Service,

    /// General messages about some kind of status.
    Status,
}

impl MessageKind {
    fn msg_color(&self) -> ColorSpec {
        match self {
            Self::Service => colors::magenta(),
            Self::Status => colors::bold_blue(),
        }
    }
}

mod colors {
    use termcolor::{Color, ColorSpec};

    pub fn bold_blue() -> ColorSpec {
        let mut out = blue();
        out.set_intense(true);
        out
    }
    pub fn blue() -> ColorSpec {
        let mut out = ColorSpec::new();
        out.set_fg(Some(Color::Blue));
        out.set_intense(true);
        out
    }
    pub fn magenta() -> ColorSpec {
        let mut out = ColorSpec::new();
        out.set_fg(Some(Color::Magenta));
        out.set_intense(true);
        out
    }
    pub fn icon() -> ColorSpec {
        let mut out = ColorSpec::new();
        out.set_fg(Some(Color::Yellow));
        out.set_intense(true);
        out
    }
    pub fn prefix() -> ColorSpec {
        let mut out = ColorSpec::new();
        out.set_fg(Some(Color::Green));
        out.set_intense(true);
        out
    }
}
