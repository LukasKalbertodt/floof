Floof
=====

## Floof is not developed anymore!

> Take a look at [penguin](https://github.com/LukasKalbertodt/penguin) instead! It provides the dev-server that floof used.

The main reason I have given up on this project is that I now think that build scripts should be written in some kind of real scripting/programming language. 
And not as a YAML file, where floof defines a really bad "scripting language" on top.

I worked with floof for quite a while, always making adjustments to its "language"/syntax/format. 
And I certainly liked some aspects of it. 
But I always found myself wanting more ways to abstract or deduplicate logic. 
And even with "tasks" in floof that could be called by other tasks, that would still come up a lot. 
And I would want to do lots of things that floof just could not yet do.
So either I had to implement it in floof, making it larger, or call external programs.
Using a real, established programming/scripting language solves most of these problems.

Also, floof just turned out to be quite a large project.
And I already have more projects than I can chew!
So I need to stop spending times on the ones that are not that important to me.


<br>

---

## Old README

[<img alt="CI status of master" src="https://img.shields.io/github/workflow/status/LukasKalbertodt/floof/CI/master?label=CI&logo=github&logoColor=white&style=for-the-badge" height="23">](https://github.com/LukasKalbertodt/floof/actions?query=workflow%3ACI+branch%3Amaster)
[<img alt="Crates.io Version" src="https://img.shields.io/crates/v/floof?logo=rust&style=for-the-badge" height="23">](https://crates.io/crates/floof)
<img alt="Crates.io Downloads" src="https://img.shields.io/crates/d/floof?color=%233498db&label=crates.io%20downloads&style=for-the-badge" height="23">


Floof is a language and tech-stack agnostic, simple-to-use development server, file-watcher and tiny build system.
It is mainly useful for web-development (i.e. where you inspect your software in the browser) due to its ability to automatically reload your app in the browser.
For other projects, [cargo watch](https://github.com/passcod/cargo-watch) or [watchexec](https://github.com/watchexec/watchexec) might be better suited (and those are way more mature).

**Features**:

- [x] Run arbitrary commands
- [x] Watch for file changes (with debouncing)
- [x] Automatically reload the page in your browser
- [x] HTTP server
    - [x] Reverse-proxy (usually to your backend application)
    - [x] Inject JS code for "auto reload"
    - [ ] Static file server
- [x] Tiny build-system
- [ ] Platform-independent file system operations (copy, ...)
- [ ] Templates to support zero-configuration use in some situations


## Installation

Currently the best way is to install from `crates.io`.
You need Rust and Cargo to do that, as you compile the application yourself.

```
cargo install floof
```

At some point I will start attaching pre-compiled binaries to the GitHub releases.


## Example

A `floof.yaml` is expected in the root folder of the project/in the directory you run `floof` in (like `Makefile`).
That file defines what actions need to be run and configures a bunch of other stuff.
The following is an example for a simple project that uses a Rust backend server that serves HTML and listens on port 3030.

```yaml
default:
  - concurrently:
    - http:
        proxy: 127.0.0.1:3030
    - watch:
        paths:
          - Cargo.toml
          - Cargo.lock
          - src/
        run:
          - reload:      # This will wait for port 3030 to become available
          - cargo run    # This long running command is killed on file changes
```

When running `floof` in that directory, the output looks something like this:

```
════════════╡    [default][http] Listening on http://127.0.0.1:8030
════════════╡ ▶️  [default][command] running: cargo run
   Compiling floof-example v0.0.0
    Finished dev [unoptimized + debuginfo] target(s) in 2.21s
     Running `target/debug/floof-example`

... output from your server application ...

════════════╡ ♻️  [default][http] Reloading all active sessions
```

You are then supposed to open `localhost:8030` in your browser.
This will show exactly the same as your actual backend server (which is listening on `localhost:3030`) as `floof` works as a reverse proxy.
However, `floof` injects a small JS snippet responsible for automatically reloading the page in your browser once something changes.
This snippet communicates with `floof` via web sockets.

When changing a file:

```
════════════╡ 🛑 [default][watch] change detected while executing operations! Cancelling operations, then debouncing for 500ms...
════════════╡ 🔥 [default][watch] change detected: running all operations...
════════════╡ ▶️  [default][command] running: cargo run

... output from your server application ...

════════════╡ ♻️  [default][http] Reloading all active sessions
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
