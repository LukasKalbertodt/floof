use std::{
    io::{self, Write},
    sync::{atomic::{Ordering, AtomicBool}, mpsc::Sender, Arc}, fmt, net::SocketAddr,
};
use anyhow::Error;
use termcolor::{Buffer, BufferWriter, Color, ColorChoice, ColorSpec, WriteColor};
use crate::config::Command;


#[derive(Clone)]
pub struct Ui {
    writer: Arc<BufferWriter>,
    errors: Sender<Error>,
    at_start_of_line: Arc<AtomicBool>,
}

const PREFIX: &str = "════════";


fn bold_blue() -> ColorSpec {
    let mut out = blue();
    out.set_intense(true);
    out
}
fn blue() -> ColorSpec {
    let mut out = ColorSpec::new();
    out.set_fg(Some(Color::Blue));
    out.set_intense(true);
    out
}
fn magenta() -> ColorSpec {
    let mut out = ColorSpec::new();
    out.set_fg(Some(Color::Magenta));
    out.set_intense(true);
    out
}
fn icon() -> ColorSpec {
    let mut out = ColorSpec::new();
    out.set_fg(Some(Color::Yellow));
    out.set_intense(true);
    out
}

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
        self.print_line(false, |buf| {
            buf.set_color(&magenta())?;
            write!(buf, "{}", PREFIX)?;
            buf.set_color(&icon())?;
            write!(buf, " […] ")?;
            buf.set_color(&magenta())?;
            write!(buf, "watching for '{}': ", action)?;

            const MAX_PATHS: usize = 3;
            for (i, path) in paths.iter().enumerate() {
                match i {
                    0 => write!(buf, "`{}`", path)?,
                    i if i >= MAX_PATHS => {
                        writeln!(buf, " and {} more", paths.len() - MAX_PATHS)?;
                        break;
                    }
                    i if i == paths.len() - 1 => writeln!(buf, " and `{}`", path)?,
                    _ => write!(buf, ", `{}`", path)?,
                }
            }

            Ok(())
        });
    }

    pub fn listening(&self, addr: &SocketAddr) {
        self.print_line(false, |buf| {
            buf.set_color(&magenta())?;
            write!(buf, "{}", PREFIX)?;
            buf.set_color(&icon())?;
            write!(buf, " […] ")?;
            buf.set_color(&magenta())?;
            writeln!(buf, "listening on 'http://{}'", addr)?;

            Ok(())
        });
    }

    pub fn listening_ws(&self, addr: &SocketAddr) {
        self.print_line(false, |buf| {
            buf.set_color(&magenta())?;
            write!(buf, "{}", PREFIX)?;
            buf.set_color(&icon())?;
            write!(buf, " […] ")?;
            buf.set_color(&magenta())?;
            writeln!(buf, "websockets listening on 'ws://{}'", addr)?;

            Ok(())
        });
    }

    pub fn exiting_no_watcher(&self) {
        self.print_line(false, |buf| {
            buf.set_color(&bold_blue())?;
            write!(buf, "{}", PREFIX)?;
            buf.set_color(&icon())?;
            write!(buf, " […] ")?;
            buf.set_color(&bold_blue())?;
            writeln!(buf, "no HTTP server or watcher configured: we are done already! Bye :)")?;

            Ok(())
        });
    }

    pub fn change_detected(&self, action: &str) {
        self.print_line(false, |buf| {
            buf.set_color(&bold_blue())?;
            write!(buf, "{}", PREFIX)?;
            buf.set_color(&icon())?;
            write!(buf, " […] ")?;
            buf.set_color(&bold_blue())?;
            write!(buf, "change detected for action '{}', debouncing...", action)?;

            Ok(())
        });
    }

    pub fn run_on_change_handlers(&self, action: &str) {
        self.print_line(true, |buf| {
            buf.set_color(&bold_blue())?;
            write!(buf, "{}", PREFIX)?;
            buf.set_color(&icon())?;
            write!(buf, " [·] ")?;
            buf.set_color(&bold_blue())?;
            writeln!(
                buf,
                "change detected for action '{}', executing 'on_change' handler...",
                action,
            )?;

            Ok(())
        });
    }

    pub fn run_command(&self, handler: &str, command: &Command) {
        self.print_line(false, |buf| {
            buf.set_color(&blue())?;
            write!(buf, "{}", PREFIX)?;
            buf.set_color(&icon())?;
            write!(buf, " [▶] ")?;
            buf.set_color(&blue())?;
            writeln!(buf, "running ({}): {}", handler, command)?;

            Ok(())
        });
    }

    fn print_line(
        &self,
        replace_line: bool,
        f: impl FnOnce(&mut Buffer) -> Result<(), Error>,
    ) {
        let mut buf = self.writer.buffer();

        // TODO: use try-catch syntax here if it ever gets stabilized.
        let result = (|| -> Result<(), Error> {
            if !replace_line && !self.at_start_of_line.load(Ordering::SeqCst) {
                writeln!(buf)?;
            }
            if replace_line {
                write!(buf, "\r")?;
            }

            f(&mut buf)?;
            buf.reset()?;
            self.writer.print(&buf)?;
            io::stdout().flush()?;

            Ok(())
        })();

        if let Err(e) = result {
            // We ignore the error: if the channel is dropped, all threads will
            // soon be removed anyway.
            let _ = self.errors.send(e);
        }
    }
}


// ╞════════════╡
// ════════════╡ Peter
// ━━━┝ ┣ ┫  ┥
// ━━━━━━━━━━━━┥ Peter
