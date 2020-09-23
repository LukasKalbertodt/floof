use std::io::Write;
use once_cell::sync::OnceCell;
use bunt::termcolor::{Buffer, BufferWriter};
use crate::{
    Args, Context,
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

pub fn if_verbose(f: impl FnOnce()) {
    let level = *crate::ui::VERBOSITY.get().expect("bug: ui not initialized yet");
    if level == crate::ui::Verbosity::Verbose || level == crate::ui::Verbosity::Trace {
        f()
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
    ctx: &Context,
    op: Option<&str>,
) -> Result<(), std::io::Error> {
    bunt::write!(
        buf,
        "{[blue]} {} {$black+intense}[{[blue+intense+bold]}]{/$}",
        PREFIX,
        icon,
        ctx.frame_label()
    )?;

    if let Some(op) = op {
        bunt::write!(buf, "{$black+intense}[{[blue+intense]}]{/$}", op)?;
    }

    write!(buf, " ")?;

    Ok(())
}

/// Emit a message.
macro_rules! msg {
    (@to_option -) => { None };
    (@to_option [$inner:expr]) => { Some($inner) };

    // Still unused: 📸 🔔 💧 ⚡ ❄ 🌊 🌈 🌀 ⏳ ⌛ 💡 👂 👋
    (@icon -) => { "  " };
    (@icon info) => { "ℹ️ " };
    (@icon warn) => { "⚠️ " };
    (@icon fire) => { "🔥" };
    (@icon run) => { "▶️ " };
    (@icon reload) => { "♻️ " };
    (@icon eye) => { "👁 " };
    (@icon stop) => { "🛑" };
    (@icon waiting) => { "⏳" };
    (@icon $other:tt) => { $other };

    ($icon:tt [$task:expr] $op:tt $($t:tt)*) => {{
        let w = crate::ui::WRITER.get().expect("bug: ui not initialized yet");
        let mut buf = w.buffer();
        (|| -> Result<(), std::io::Error> {
            crate::ui::print_prefix(
                &mut buf,
                msg!(@icon $icon),
                &$task,
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
    ($($t:tt)*) => {
        crate::ui::if_verbose(|| msg!($($t)*));
    };
}
