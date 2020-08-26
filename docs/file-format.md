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


## Execution context/variables

TODO

### Working directory

TODO

<br>
<br>

## Operations

Operations are at the core of floof: they do the actual work. An "operation"
is a very general abstraction, meaning that lots of things can be implemented as
operation.

### `command`

| Async | Cancelable |
| ----- | ---------- |
| ✅    | ✅         |

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
  By default, the working directory of the current execution context is used.
  See [the "working directory" section](#working-directory) for more details.


### `copy`

TODO

### `watch`

| Async | Cancelable |
| ----- | ---------- |
| ✅    | ✅         |

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

TODO

### `on-change`

| Async | Cancelable |
| ----- | ---------- |
| ❌    | ❌         |

Only executes another operation if the operation was triggered by a file system
change. Can only be used as part of a `watch` operation.

TODO

### `set-workdir`

| Async | Cancelable |
| ----- | ---------- |
| ❌    | ❌         |

TODO
