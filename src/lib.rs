/// Unwraps a `Result`, writing a message to stderr and returning an `ExecutionError` on failure.
#[macro_export]
macro_rules! cmd_try {
    ($i:expr, $r:expr, $m:expr) => {
        match $r {
            Ok(v) => v,
            Err(e) => {
                write!($i.error(), $m).unwrap();
                return CommandResult::ExecutionError(Some(Box::new(e)));
            },
        }
    }
}

/// Unwraps an `Option`, writing a message to stderr and returning an `ExecutionError` on failure.
#[macro_export]
macro_rules! cmd_expect {
    ($i:expr, $r:expr, $m:expr) => {
        match $r {
            Some(v) => v,
            None => {
                write!($i.error(), $m).unwrap();
                return CommandResult::ExecutionError(None);
            },
        }
    }
}

extern crate io_providers;

use std::collections::HashMap;
use std::error;
use std::fmt;
use std::io::Write;
use std::iter::IntoIterator;
use io_providers::stream;

const SUCCESS_EXIT_CODE: i32 = 0;
const ARGUMENT_ERROR_EXIT_CODE: i32 = 1;
const EXECUTION_ERROR_EXIT_CODE: i32 = 2;

/// Describes an application and the commands it supports.
pub struct Application<'c, 'p:'c> {
    /// The name of the application.
    pub name: &'static str,

    /// A collection of commands the application supports.
    pub commands: &'c [Command<'p>],
}

impl<'c, 'p> Application<'c, 'p> {
    /// Prints usage information for the application.
    pub fn print_usage(&self, sp: &mut stream::Provider) {
        writeln!(sp.error(), "Usage: {} COMMAND [ARGS]\n", self.name).unwrap();
        writeln!(sp.error(), "commands:").unwrap();

        for cmd in self.commands {
            cmd.print_short_desc(sp);
        }
    }

    /// Given the command-line arguments, parses them and runs a command if applicable.
    ///
    /// Returns the error code with which to exit, and a reference to the invoked
    /// command if one was invoked.
    pub fn run(&self, sp: &mut stream::Provider, args: Vec<String>)
        -> (i32, Option<&'c Command<'p>>)
    {
        if args.len() <= 1 {
            self.print_usage(sp);
            return (ARGUMENT_ERROR_EXIT_CODE, None);
        }

        let cmd_str = args[1].clone();

        for cmd in self.commands {
            if cmd_str == cmd.name {
                let arguments = match Arguments::new(cmd.params, args) {
                    Some(a) => a,
                    None => {
                        cmd.print_usage(sp, self.name);
                        return (ARGUMENT_ERROR_EXIT_CODE, Some(cmd));
                    },
                };

                let result = (cmd.handler)(sp, &arguments);

                let exit_code = match result {
                    Success => SUCCESS_EXIT_CODE,
                    ArgumentError => {
                        cmd.print_usage(sp, self.name);
                        ARGUMENT_ERROR_EXIT_CODE
                    },
                    ExecutionError(err_opt) => {
                        if let Some(err) = err_opt {
                            writeln!(sp.error(), "Inner error: {}", err.description()).unwrap();
                        }

                        EXECUTION_ERROR_EXIT_CODE
                    },
                };

                return (exit_code, Some(cmd));
            }
        }

        writeln!(sp.error(), "Error: Unrecognized command '{}'", cmd_str).unwrap();
        (ARGUMENT_ERROR_EXIT_CODE, None)
    }
}

/// Describes a command along with how to execute it and display help info for it.
pub struct Command<'p> {
    /// The name of the command.
    pub name: &'static str,

    /// A one-line description of what the command does.
    pub short_desc: &'static str,

    /// A description of the parameters the command takes.
    pub params: &'p [Parameter],

    /// A function which, given the command arguments and i/o handles, executes the command.
    pub handler: fn(&mut stream::Provider, &Arguments) -> CommandResult,
}

impl<'p> Command<'p> {
    pub fn print_usage(&self, sp: &mut stream::Provider, app_name: &str) {
        writeln!(sp.error(), "Usage: {} {}", app_name, self).unwrap();
    }

    pub fn print_short_desc(&self, sp: &mut stream::Provider) {
        writeln!(sp.error(), "{: <22}  {}", self.name, self.short_desc).unwrap();
    }
}

/// Describes the errors which can result from a command invocation.
pub enum CommandResult {
    /// The command completed successfully.
    Success,
    /// The command was invoked incorrectly.
    ArgumentError,
    /// An error occurred while executing the command.
    ExecutionError(Option<Box<error::Error>>),
}
use CommandResult::*;

impl<'p> fmt::Display for Command<'p> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(f.write_str(self.name));

        for param in self.params {
            try!(write!(f, " {}", param));
        }

        Ok(())
    }
}

/// Describes a command parameter and how to display help info for it.
#[derive(Eq, PartialEq, Hash)]
pub struct Parameter {
    pub name: &'static str,
    pub required: bool,
    pub repeating: bool,
}

impl fmt::Display for Parameter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match (self.required, self.repeating) {
            (false, false) => write!(f, "[{}]",    self.name),
            (false, true)  => write!(f, "[{}]...", self.name),
            (true, false)  => write!(f, "{}",      self.name),
            (true, true)   => write!(f, "{}...",   self.name),
        }
    }
}

/// Describes the arguments to a command.
pub struct Arguments<'a> {
    /// A mapping from `Parameter` to the associated arguments for that parameter.
    param_to_args: HashMap<&'a Parameter, Vec<String>>,
}

impl<'a> Arguments<'a> {
    pub fn for_param(&self, param: &Parameter) -> &Vec<String> {
        &self.param_to_args[param]
    }

    /// Constructs a new `Arguments`, yielding `None` if the arguments do not
    /// match the provided parameter specification.
    fn new<'b>(params: &'b [Parameter], args: Vec<String>) -> Option<Arguments<'b>> {
        let mut param_to_args: HashMap<&Parameter, Vec<String>> = HashMap::new();
        let mut min_remaining = params.iter().filter(|p| p.required).count();
        let mut remaining = args.len() - 2;
        let mut args_iter = args.into_iter();

        // Pop the application name and command off the iterator
        args_iter.next().unwrap();
        args_iter.next().unwrap();

        for param in params {
            if remaining < min_remaining {
                return None;
            }

            if param.required {
                min_remaining = min_remaining - 1;
            }

            // Have to loop here instead of using .take(x).collect() because Vec::IntoIter
            // isn't clonable
            let param_args_count =
                if remaining == min_remaining {
                    0
                } else {
                    if param.repeating { remaining - min_remaining } else { 1 }
                };
            let mut param_args = Vec::with_capacity(param_args_count);
            for _ in 0..param_args_count {
                param_args.push(args_iter.next().unwrap());
            }
            remaining = remaining - param_args_count;

            param_to_args.insert(param, param_args);
        }

        if remaining > 0 {
            None
        } else {
            Some(Arguments { param_to_args: param_to_args })
        }
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use std::io;
    use io_providers::stream;

    #[test]
    fn application__print_usage__success() {
        let mut sp = stream::Logger::new(Vec::new());
        let params1: [Parameter; 2] = [
            Parameter { name: "PARAM1", required: true, repeating: true },
            Parameter { name: "PARAM2", required: false, repeating: false }];
        let params2: [Parameter; 0] = [];
        let cmds: [Command; 2] = [
            Command { name: "cmd1", short_desc: "desc1", params: &params1, handler: dummy_success_handler },
            Command { name: "cmd2", short_desc: "desc2", params: &params2, handler: dummy_success_handler }];
        let app: Application = Application { name: "app", commands: &cmds };
        let expected = format!("\
            Usage: app COMMAND [ARGS]\n\n\
            commands:\n\
            cmd1                    desc1\n\
            cmd2                    desc2\n");

        app.print_usage(&mut sp);

        assert_eq!(0, sp.get_output().len());
        assert_eq!(&expected, ::std::str::from_utf8(sp.get_error()).unwrap());
    }

    #[test]
    fn application__run__empty_args__prints_usage() {
        let args = vec!["app".to_string()];

        let sp = test_application_run(1, None, args);

        assert_eq!(0, sp.get_output().len());
        assert_eq!("\
            Usage: app COMMAND [ARGS]\n\n\
            commands:\n\
            cmd1                    desc1\n\
            cmd2                    desc2\n\
            cmd3                    desc3\n\
            cmd4                    desc4\n",
            ::std::str::from_utf8(sp.get_error()).unwrap());
    }

    #[test]
    fn application__run__invalid_command__prints_unrecognized_command() {
        let args = vec!["app".to_string(), "badcmd".to_string()];

        let sp = test_application_run(1, None, args);

        assert_eq!(
            "Error: Unrecognized command 'badcmd'\n",
            ::std::str::from_utf8(sp.get_error()).unwrap());
    }

    #[test]
    fn application__run__invalid_args__prints_usage() {
        let args = vec!["app".to_string(), "cmd1".to_string()];

        let sp = test_application_run(1, Some("cmd1"), args);

        assert_eq!(0, sp.get_output().len());
        assert_eq!(
            "Usage: app cmd1 param1\n",
            ::std::str::from_utf8(sp.get_error()).unwrap());
    }

    #[test]
    fn application__run__handler_success__success() {
        let args = vec!["app".to_string(), "cmd1".to_string(), "arg1".to_string()];

        let sp = test_application_run(0, Some("cmd1"), args);

        assert_eq!(0, sp.get_output().len());
        assert_eq!(0, sp.get_error().len());
    }

    #[test]
    fn application__run__handler_arg_error__prints_usage() {
        let args = vec!["app".to_string(), "cmd2".to_string(), "arg1".to_string()];

        let sp = test_application_run(1, Some("cmd2"), args);

        assert_eq!(0, sp.get_output().len());
        assert_eq!(
            "Usage: app cmd2 param1\n",
            ::std::str::from_utf8(sp.get_error()).unwrap());
    }

    #[test]
    fn application__run__handler_exec_error__success() {
        let args = vec!["app".to_string(), "cmd3".to_string(), "arg1".to_string()];

        let sp = test_application_run(2, Some("cmd3"), args);

        assert_eq!(0, sp.get_output().len());
        assert_eq!(0, sp.get_error().len());
    }

    #[test]
    fn application__run__handler_exec_error_with_inner__prints_inner() {
        let args = vec!["app".to_string(), "cmd4".to_string(), "arg1".to_string()];

        let sp = test_application_run(2, Some("cmd4"), args);

        assert_eq!(0, sp.get_output().len());
        assert_eq!(
            "Inner error: :(\n",
            ::std::str::from_utf8(sp.get_error()).unwrap());
    }

    #[test]
    fn command__display__success() {
        let params: [Parameter; 2] = [
            Parameter { name: "PARAM1", required: true, repeating: true },
            Parameter { name: "PARAM2", required: false, repeating: false }];
        let cmd = Command { name: "cmd", short_desc: "desc", params: &params, handler: dummy_success_handler };
        let expected = format!("cmd {} {}", params[0], params[1]);

        let result = format!("{}", cmd);

        assert_eq!(expected, result);
    }

    #[test]
    fn command__print_usage__success() {
        let mut sp = stream::Logger::new(Vec::new());
        let params: [Parameter; 0] = [];
        let cmd = Command { name: "cmd", short_desc: "desc", params: &params, handler: dummy_success_handler };
        let expected = format!("Usage: app {}\n", cmd);

        cmd.print_usage(&mut sp, "app");

        assert_eq!(0, sp.get_output().len());
        assert_eq!(&expected, ::std::str::from_utf8(sp.get_error()).unwrap());
    }

    #[test]
    fn command__print_short_desc__success() {
        let mut sp = stream::Logger::new(Vec::new());
        let params: [Parameter; 0] = [];
        let cmd = Command { name: "cmd", short_desc: "the short desc", params: &params, handler: dummy_success_handler };
        let expected = "cmd                     the short desc\n".to_string();

        cmd.print_short_desc(&mut sp);

        assert_eq!(0, sp.get_output().len());
        assert_eq!(&expected.into_bytes()[..], sp.get_error());
    }

    #[test]
    fn parameter__display_optional_nonrepeating__success() {
        let param = Parameter { name: "PARAM", required: false, repeating: false };
        test_param_display("[PARAM]", &param);
    }

    #[test]
    fn parameter__display_optional_repeating__success() {
        let param = Parameter { name: "PARAM", required: false, repeating: true };
        test_param_display("[PARAM]...", &param);
    }

    #[test]
    fn parameter__display_required_nonrepeating__success() {
        let param = Parameter { name: "PARAM", required: true, repeating: false };
        test_param_display("PARAM", &param);
    }

    #[test]
    fn parameter__display_required_repeating__success() {
        let param = Parameter { name: "PARAM", required: true, repeating: true };
        test_param_display("PARAM...", &param);
    }

    #[test]
    fn arguments__new__too_few_args__returns_none() {
        let param = Parameter { name: "PARAM", required: true, repeating: false };
        let params = &[param];
        let args = vec!["app".to_string(), "cmd".to_string()];

        let result = Arguments::new(params, args);

        assert!(result.is_none());
    }

    #[test]
    fn arguments__new__too_many_args__returns_none() {
        let param = Parameter { name: "PARAM", required: true, repeating: false };
        let params = &[param];
        let args = vec!["app".to_string(), "cmd".to_string(), "arg1".to_string(), "arg2".to_string()];

        let result = Arguments::new(params, args);

        assert!(result.is_none());
    }

    #[test]
    fn arguments__new__optional_param_and_no_args__returns_empty() {
        let params = &[Parameter { name: "PARAM", required: false, repeating: false }];
        let args = vec!["app".to_string(), "cmd".to_string()];

        let arguments = Arguments::new(params, args).unwrap();

        assert_eq!(0, arguments.for_param(&params[0]).len());
    }

    #[test]
    fn arguments__new__required__success() {
        let params = &[
            Parameter { name: "PARAM1", required: true, repeating: false },
            Parameter { name: "PARAM2", required: true, repeating: false }];
        let (arg1, arg2) = ("arg1".to_string(), "arg2".to_string());
        let args = vec!["app".to_string(), "cmd".to_string(), arg1.clone(), arg2.clone()];

        let arguments = Arguments::new(params, args).unwrap();

        assert_eq!(&vec![arg1], arguments.for_param(&params[0]));
        assert_eq!(&vec![arg2], arguments.for_param(&params[1]));
    }

    #[test]
    fn arguments__new__repeating_param_and_args__success() {
        let params = &[Parameter { name: "PARAM", required: true, repeating: true }];
        let (arg1, arg2) = ("arg1".to_string(), "arg2".to_string());
        let args = vec!["app".to_string(), "cmd".to_string(), arg1.clone(), arg2.clone()];

        let arguments = Arguments::new(params, args.clone()).unwrap();

        assert_eq!(&vec![arg1, arg2], arguments.for_param(&params[0]));
    }

    #[test]
    fn arguments__new__repeating_then_required__success() {
        let params = &[
            Parameter { name: "PARAM1", required: true, repeating: true },
            Parameter { name: "PARAM2", required: true, repeating: false }];
        let (arg1, arg2, arg3) = ("arg1".to_string(), "arg2".to_string(), "arg3".to_string());
        let args = vec!["app".to_string(), "cmd".to_string(), arg1.clone(), arg2.clone(), arg3.clone()];

        let arguments = Arguments::new(params, args).unwrap();

        assert_eq!(&vec![arg1, arg2], arguments.for_param(&params[0]));
        assert_eq!(&vec![arg3], arguments.for_param(&params[1]));
    }

    #[test]
    fn arguments__new__required_then_repeating__success() {
        let params = &[
            Parameter { name: "PARAM", required: true, repeating: false },
            Parameter { name: "PARAM", required: true, repeating: true }];
        let (arg1, arg2, arg3) = ("arg1".to_string(), "arg2".to_string(), "arg3".to_string());
        let args = vec!["app".to_string(), "cmd".to_string(), arg1.clone(), arg2.clone(), arg3.clone()];

        let arguments = Arguments::new(params, args).unwrap();

        assert_eq!(&vec![arg1], arguments.for_param(&params[0]));
        assert_eq!(&vec![arg2, arg3], arguments.for_param(&params[1]));
    }

    #[test]
    fn arguments__new__optional_then_required_with_one_arg__success() {
        let params = &[
            Parameter { name: "PARAM1", required: false, repeating: false },
            Parameter {  name: "PARAM2", required: true, repeating: false }];
        let arg1 = "arg1".to_string();
        let args = vec!["app".to_string(), "cmd".to_string(), arg1.clone()];

        let arguments = Arguments::new(params, args.clone()).unwrap();

        assert_eq!(0, arguments.for_param(&params[0]).len());
        assert_eq!(&vec![arg1], arguments.for_param(&params[1]));
    }

    #[test]
    fn arguments__new__optional_then_required_with_two_args__success() {
        let params = &[
            Parameter { name: "PARAM1", required: false, repeating: false },
            Parameter { name: "PARAM2", required: true, repeating: false }];
        let (arg1, arg2) = ("arg1".to_string(), "arg2".to_string());
        let args = vec!["app".to_string(), "cmd".to_string(), arg1.clone(), arg2.clone()];

        let arguments = Arguments::new(params, args).unwrap();

        assert_eq!(&vec![arg1], arguments.for_param(&params[0]));
        assert_eq!(&vec![arg2], arguments.for_param(&params[1]));
    }

    #[test]
    fn arguments__new__required_then_optional_with_one_arg__success() {
        let params = &[
            Parameter { name: "PARAM1", required: true, repeating: false },
            Parameter { name: "PARAM2", required: false, repeating: false }];
        let arg1 = "arg1".to_string();
        let args = vec!["app".to_string(), "cmd".to_string(), arg1.clone()];

        let arguments = Arguments::new(params, args.clone()).unwrap();

        assert_eq!(&vec![arg1], arguments.for_param(&params[0]));
        assert_eq!(0, arguments.for_param(&params[1]).len());
    }

    #[test]
    fn arguments__new__required_then_optional_with_two_args__success() {
        let params = &[
            Parameter { name: "PARAM1", required: true, repeating: false },
            Parameter { name: "PARAM2", required: false, repeating: false }];
        let (arg1, arg2) = ("arg1".to_string(), "arg2".to_string());
        let args = vec!["app".to_string(), "cmd".to_string(), arg1.clone(), arg2.clone()];

        let arguments = Arguments::new(params, args).unwrap();

        assert_eq!(&vec![arg1], arguments.for_param(&params[0]));
        assert_eq!(&vec![arg2], arguments.for_param(&params[1]));
    }

    fn test_application_run(
        expected_exit_code: i32,
        expected_cmd_name: Option<&str>,
        args: Vec<String>)
        -> stream::Logger
    {
        let mut sp = stream::Logger::new(Vec::new());
        let app = Application {
            name: "app",
            commands: &[
                Command {
                    name: "cmd1",
                    short_desc: "desc1",
                    params: &[
                        Parameter {
                            name: "param1",
                            required: true,
                            repeating: false,
                        },
                    ],
                    handler: dummy_success_handler,
                },
                Command {
                    name: "cmd2",
                    short_desc: "desc2",
                    params: &[
                        Parameter {
                            name: "param1",
                            required: true,
                            repeating: false,
                        },
                    ],
                    handler: dummy_arg_error_handler,
                },
                Command {
                    name: "cmd3",
                    short_desc: "desc3",
                    params: &[
                        Parameter {
                            name: "param1",
                            required: true,
                            repeating: false,
                        },
                    ],
                    handler: dummy_exec_error_handler,
                },
                Command {
                    name: "cmd4",
                    short_desc: "desc4",
                    params: &[
                        Parameter {
                            name: "param1",
                            required: true,
                            repeating: false,
                        },
                    ],
                    handler: dummy_exec_error_with_inner_handler,
                },
            ],
        };

        let (exit_code, cmd_opt) = app.run(&mut sp, args);

        assert_eq!(expected_exit_code, exit_code);
        match expected_cmd_name {
            Some(n) => assert_eq!(n, cmd_opt.unwrap().name),
            None => assert!(cmd_opt.is_none()),
        }

        sp
    }

    fn test_param_display(expected: &str, param: &Parameter) {
        let result = format!("{}", param);
        assert_eq!(expected, &result);
    }

    #[allow(unused_variables)]
    fn dummy_success_handler(sp: &mut stream::Provider, args: &Arguments) -> CommandResult {
        CommandResult::Success
    }

    #[allow(unused_variables)]
    fn dummy_arg_error_handler(sp: &mut stream::Provider, args: &Arguments) -> CommandResult {
        CommandResult::ArgumentError
    }

    #[allow(unused_variables)]
    fn dummy_exec_error_handler(sp: &mut stream::Provider, args: &Arguments) -> CommandResult {
        CommandResult::ExecutionError(None)
    }

    #[allow(unused_variables)]
    fn dummy_exec_error_with_inner_handler(sp: &mut stream::Provider, args: &Arguments) -> CommandResult {
        CommandResult::ExecutionError(Some(Box::new(io::Error::new(io::ErrorKind::Other, ":("))))
    }
}
