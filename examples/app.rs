extern crate command_cli;
extern crate io_providers;

use std::env;
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

#[allow(unused_variables)]
fn cmd1_handler(sp: &mut stream::Provider, args: &Arguments) -> CommandResult {
    // TODO: fill in these handlers, using cmd_try! and cmd_expect!
    CommandResult::Success
}

#[allow(unused_variables)]
fn cmd2_handler(sp: &mut stream::Provider, args: &Arguments) -> CommandResult {
    CommandResult::ArgumentError
}

#[allow(unused_variables)]
fn cmd3_handler(sp: &mut stream::Provider, args: &Arguments) -> CommandResult {
    CommandResult::ExecutionError(None)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut sp = stream::Std::new();
    let (exit_code, _) = APP.run(&mut sp, args);
    process::exit(exit_code);
}
