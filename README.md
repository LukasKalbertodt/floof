Watchboi
========

[<img alt="CI status of master" src="https://img.shields.io/github/workflow/status/LukasKalbertodt/watchboi/CI/master?label=CI&logo=github&logoColor=white&style=for-the-badge" height="23">](https://github.com/LukasKalbertodt/watchboi/actions?query=workflow%3ACI+branch%3Amaster)
[<img alt="Crates.io Version" src="https://img.shields.io/crates/v/watchboi?logo=rust&style=for-the-badge" height="23">](https://crates.io/crates/watchboi)
<img alt="Crates.io Downloads" src="https://img.shields.io/crates/d/watchboi?color=%233498db&label=crates.io%20downloads&style=for-the-badge" height="23">
<img alt="GitHub Downloads" src="https://img.shields.io/github/downloads/LukasKalbertodt/watchboi/total?color=3498db&label=Github%20downloads&style=for-the-badge" height="23">


Watchboi is a language and tech-stack agnostic, simple-to-use development server, file-watcher and tiny build system.
It is mainly useful for web-development (i.e. where you inspect your software in the browser) due to its ability to automatically reload your app in the browser.
For other projects, [cargo watch](https://github.com/passcod/cargo-watch) or [watchexec](https://github.com/watchexec/watchexec) might be better suited (and those are way more mature).

**Features**:

- [x] Watch for file changes (with debouncing)
- [x] Run arbitrary commands at the start or whenever a file changes
- [x] Automatically reload the page in your browser when something changes
    - [x] Either once all commands are done (useful when the commands generate static files)
    - [x] Or once the reverse-proxy target port is open again (useful when the commands start a webserver)
- [x] HTTP server
    - [x] Reverse-proxy (usually to your backend application)
    - [x] Inject JS code for "auto reload"
    - [ ] Static file server
- [ ] Templates to support zero-configuration use in some situations
- [ ] Tiny build-system


## Installation

Currently the best way is to install from `crates.io`.
You need Rust and Cargo to do that, as you compile the application yourself.

```
cargo install watchboi
```

At some point I will start attaching pre-compiled binaries to the GitHub releases.


## Example

A `watchboi.toml` is expected in the root folder of the project/in the directory you run `watchboi` in (like `Makefile`).
That file defines what actions need to be run and configures a bunch of other stuff.
The following is an example for a project that uses a Rust backend server and a React (JS) frontend.
There are separate folders `frontend` and `backend` for the two parts of the project.

```toml
[http]
proxy = "127.0.0.1:3000"

[actions.backend]
base = "backend"
watch = ["Cargo.toml", "Cargo.lock", "src/"]
run = ["cargo run"]
reload = "early"

[actions.frontend]
base = "frontend"
watch = ["package.json", "package-lock.json", "webpack.config.js", "src/"]
run = ["npx webpack --mode=development"]
reload = "late"
```

When running `watchboi` in that directory, the output looks something like this:

```
════════════ 🌀 websockets listening on 'ws://127.0.0.1:8031'
════════════ 🌀 listening on 'http://127.0.0.1:8030'
════════════ 👁 watching for 'backend': `Cargo.toml`, `Cargo.lock` and `src/`
════════════ 🐇 running (on_start): cargo run
════════════ 👁 watching for 'frontend': `package.json`, `package-lock.json`, `webpack.config.js` and 1 more
════════════ 🐇 running (on_start): npx webpack --mode=development
   Compiling backend v0.1.0
    Finished dev [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/backend`
Backend listening on http://127.0.0.1:3000

... output from webpack ...
```

You are then supposed to open `localhost:8030` in your browser.
This will show exactly the same as your actual backend server (which is listening on `localhost:3000`) as `watchboi` works as a reverse proxy.
However, `watchboi` injects a small JS snippet responsible for automatically reloading the page in your browser once something changes.
This snippet communicates with `watchboi` via web sockets.

When changing a frontend file:

```
════════════ 🔥 change detected for action 'frontend', executing handler...
════════════ 🐇 running (on_change): npx webpack --mode=development

... webpack output ...

════════════ ↻  reloading browser (due to change in action 'frontend')
```

When changing a backend file:

```
════════════ 🔥 change detected for action 'backend', executing handler...
════════════ 🐇 running (on_change): cargo run
   Compiling backend v0.1.0
    Finished dev [unoptimized + debuginfo] target(s) in 1.24s
     Running `target/debug/backend`
Backend listening on http://127.0.0.1:3000
════════════ ↻  reloading browser (due to change in action 'backend')
```


## Status of this project

This project is really young and lots of stuff might still break!
A lot of features are missing as well.
I only started this project to help with developing another project I am working on.


---

## License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.
</sub>
