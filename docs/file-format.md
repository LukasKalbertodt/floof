# Floof Configuration file format

For floof to do anything, it requires a YAML configuration file. This is
typically `floof.yaml` in the current directory, but a different file can be
passed via the `-c` or `--config` command line parameter.

The configuration file defines a map of *tasks* at the top level, each of which
is just a list of *operations*. So the basic layout is like this:

```yaml
task-a:
  - operation1
  - operation2

task-b:
  - operation3
```

The names of the tasks can be chosen arbitrarily. However, in future versions
(with a major version bump), some names might get reserved for special purposes
(like setting global values).

There is one special task: `default`. If floof is started without any arguments,
the `default` task is started. If you want to start a non-default task, you can
run `floof run <task>`.

Running a task executes all its operations in order. If an operation fails,
execution is stopped and the remaining operations are not executed.


## Execution context

Each operation is executed in an execution context. Each context might have a
parent context, thus we are dealing with a context tree. The root of the tree is
the task that is started first (e.g. `default`). Operations that start other
operations or tasks (e.g. `watch`, `concurrently`, `run-task`) each create a new
child context for their child operations.

This is important because operations can store values inside of their execution
context. Other operations can read those values, allowing operations to
communicate with each other. Most operations will use the closest value in their
context chain (the path from their context to the root context). That means that
the current context is checked first; when it does not contain the expected
value, its parent is searched, until the value is found or the root of the
context tree is reached.

For example, the `command` operation (among others) tries to retrieve the
working directory from the nearest context. The `set-workdir` operation can be
used to set that value in the current context.

```yaml
default:
  - set-workdir: /tmp
  - pwd     # /tmp
  - run-task: foo

foo:
  # No workdir set in current context, but in parent one ('default').
  - pwd     # /tmp
  - run-task: bar

bar:
  - set-workdir: /home
  - pwd     # /home
```

<br>
<br>

## Operations

Operations are at the core of floof: they do the actual work. An "operation"
is a very general abstraction, meaning that lots of things can be implemented as
operation.

### `command`

Allows to execute an arbitrary external command.

**Example**

```yaml
default:
  - npm install
  - npm run build
  - command:
      run: python -m http.server
      workdir: dist/
```

This is probably the most important and common operation. As such, it has a
special syntax in the configuration file: it can be defined by a map (like all
other operations, too) but also by a string or an array of string. And as YAML
allows for bare strings, it might even look like four different syntaxes! All of
these four operations are exactly equivalent:

```yaml
default:
  - cargo run            # bare string, works most of the time
  - "cargo run"          # explicitly quoted, if YAML bare string doesn't work
  - ["cargo", "run"]     # array syntax, for arguments with whitespace
  - command:             # YAML map syntax
      run: cargo run     # or "cargo run" or ["cargo", "run"]
```

The explicit map form (`command: ...`) is useful to specify additional
properties about the command invocation, like working directory and environment
variables.

**Note**: the command is not executed in a shell! This means that shell features
you might be used to are not available here. This includes things like IO
redirects (e.g. `> out.txt` or `| grep foo`), setting environment variables
(e.g. `FOO=bar cargo run`) or chaining commands (e.g. `cargo build && cargo
run`).

Also note that when the command is specified as string, the string is split on
whitespace. The first part is the program name, all other parts are passed as
parameters. If your parameters have to contain whitespace, you cannot quote them
like in a shell (e.g. `grep "foo bar"`). Instead you have to use the explicit
array of strings syntax: `["grep", "foo bar"]`.

When a `command` is **cancelled**, the running process is killed (SIGKILL on
unix).

#### Configurable properties:

- `run`: the actual command. Can be specified as bare string, explicitly quoted
  string or as an array of strings.
- `workdir`: specifies the working directory in which the command is executed.
  By default, the working directory is retrieved from the nearest execution
  context that has the working directory set.



### `copy`

TODO

### `watch`

Watches directories and/or files and triggers user defined operations when a
change is detected.

**Example**

```yaml
default:
  - watch:
      paths:
        - Cargo.toml
        - Cargo.lock
        - src
      run:
        - cargo build
        - cargo test
```

The operations listed under `run` are executed:
- once at the beginning (when `watch` is started itself), and
- each time some file system changed is detected in any of the specified
  `paths`.

If you want some operation not to execute in the beginning, but only when some
change is detected, see [the operation `on-change`](#on-change).

All operations are executed like a task executes its operations. However, if a
file system change is detected before all operations have finished, the
currently running operation is *cancelled* and execution starts from the top
again. The exact cancel behavior is slightly different from operation to
operation (e.g. commands are killed).

TODO: explain path recursion
TODO: explain debouncing

#### Configurable properties:

- `run`: the list of operations.
- `paths`: a list of paths to be watched. Can be files or directories.
  Directories are watched recursively. TODO: explain "recursively" in more
  detail. TODO: implement and explain glob patterns.
- `debounce` (integer, default: `500`): the debounce duration in milliseconds.
  When a file change is detected, the operations are not triggered immediately.
  Instead, we wait for the debounce duration to see if any other changes happen.
  Whenever a new change is detected, the debounce timer is reset (so in theory,
  that could stall indefinitely). Only after no new change has been detected for
  `debounce` milliseconds, the operations are executed.

### `http`

Starts an HTTP server that can function as a reverse proxy or static file server
and can automatically reload browser sessions via `reload`.

**Example**

For this example, we assume that your application is a webserver and `cargo run`
will start it listening on port 8000. With this configuration, the floof HTTP
server works as a reverse proxy to `localhost:8000` and itself listens on port
8030 (the default). That means that your website can be viewed via
`localhost:8000` *and* `localhost:8030`.

Whenever a file in your project changes, your webserver is compiled and
restarted, and all tabs in your browser that currently show `localhost:8030`
will automatically reload.

```yaml
default:
  - concurrently:
    - http:
      - proxy: localhost:8000
    - watch:
        paths:
          - src
          - Cargo.toml
        run:
          - reload:
          - cargo run
```

If `proxy` is defined, the server functions as reverse proxy; if `serve` is
defined, it functions as a static file server. Exactly one of `proxy` or `serve`
has to be defined.

In either operation mode, the HTTP response will contain a tiny JS snippet that
is used to reload the browser session. It works like this: this `http` operation
will also listen on another port (8031 by default) for incominb websocket (WS)
connections. The JS code in the snippet will attempt to connect to that port and
keep the connection open indefinitely. When floof wants to reload all browser
sessions, it can communicate with those sessions via the websocket connection.

#### Configurable properties:

- `proxy`: a socket address denoting the target of the reverse proxy.
- `serve`: a local path that will be served by the static file server.
- `addr`: the address of the server to bind to (default: `localhost:8030`).
- `ws-addr`: the address of the websocket server to bind to (default:
  `localhost:8031`).


### `reload`

Reloads all browser sessions of the nearest `http` operation in the context
chain. Basically only makes sense inside a `watch` operation.

If the nearest `http` operation functions as a reverse proxy, it will wait until
the target port is open before reloading. So you can add a `reload:` operation
right before the operation that starts your webserver and it will be reloaded at
the correct time.


### `on-change`

Only executes another operation if the operation was triggered by a file system
change. Can only be used as part of a `watch` operation. See [`watch`
operation](#watch) for more information.

**Example**

Here, when starting floof, `echo bar` is executed. Then, whenever one of the
watched files changes, `echo foo` followed by `echo bar` is executed.

```yaml
default:
  - watch:
      paths:
        - src
        - package.json
      run:
        - on-change: echo foo
        - echo bar
```


### `set-workdir`

Sets the working directory in the current execution context. Operations that are
executed after this operation in the current context or any child contexts use
this working directory. That is, unless the working directory is overwritten
(e.g. by another `set-workdir` operation).

The starting working directory is always the directory of the configuration
file.

**Example**

```yaml
default:
  - make          # executed in same directory as config file
  - set-workdir: build
  - strip app     # executed in '{config-file-dir}/build'
```

There are different behaviors for different paths:

- If the given path is absolute, that exact path is used.
- If the given path starts with `./`, it is appended to the current working
  directory.
- Otherwise, the path is appended to the path of the configuration file (minus
  file name).
