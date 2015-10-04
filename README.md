# command-cli [![Build Status](https://travis-ci.org/pshendry/command-cli.svg)](https://travis-ci.org/pshendry/command-cli)

A library for building CLI applications in Rust which have a
command-based interface (like git or apt-get).

## Example

`Cargo.toml`:

```
[dependencies]
command-cli = "0.1"
```

`src/main.rs`

```rust
#[macro_use(cmd_try, cmd_expect)]
extern crate command_cli;
extern crate io_providers;

use std::env;
use std::io::Write;
use std::process;
use command_cli::{Application, Arguments, Command, CommandResult, Parameter, StaticApplication};
use io_providers::stream;

const APP: StaticApplication = Application {
    name: "app",
    commands: &[
        Command {
            name: "cmd1",
            short_desc: "foos the bars via extensible frameworks",
            params: &[
                Parameter {
                    name: "FOO",
                    required: true,
                    repeating: false,
                },
                Parameter {
                    name: "BAR",
                    required: true,
                    repeating: true,
                },
            ],
            handler: cmd1_handler,
        },
        Command {
            name: "cmd2",
            short_desc: "executes command #2 on the thing",
            params: &[
                Parameter {
                    name: "THING",
                    required: false,
                    repeating: false,
                },
            ],
            handler: cmd2_handler,
        },
        Command {
            name: "cmd3",
            short_desc: "runs command #3 on the files",
            params: &[
                Parameter {
                    name: "FILE",
                    required: false,
                    repeating: true,
                },
            ],
            handler: cmd3_handler,
        },
    ],
};

fn cmd1_handler(sp: &mut stream::Provider, args: &Arguments) -> CommandResult {
    let foo: &String = &args["FOO"][0];
    let bars: &Vec<String> = &args["BAR"];
    let home_dir = cmd_expect!(sp, env::home_dir(), "Error: Unable to get home directory");
    CommandResult::Success
}

fn cmd2_handler(sp: &mut stream::Provider, args: &Arguments) -> CommandResult {
    let thing: Option<&String> = args["THING"].iter().next();
    let var = cmd_try!(sp, env::var("ENV_VAR"), "Error: Unable to get 'ENV_VAR' environment variable");
    CommandResult::ArgumentError
}

fn cmd3_handler(sp: &mut stream::Provider, args: &Arguments) -> CommandResult {
    CommandResult::ExecutionError(None)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut sp = stream::Std::new();
    let (exit_code, _) = APP.run(&mut sp, args);
    process::exit(exit_code);
}
```

## License

`command-cli` is distributed under the [MIT license](https://opensource.org/licenses/MIT).
