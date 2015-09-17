extern crate io_providers;

use std::collections::HashMap;
use std::error;
use std::fmt;
use std::iter::IntoIterator;
use io_providers::StreamProvider;

pub struct CommandManager<'a> {
    cmds: &'a [Command],
}

impl<'a> CommandManager<'a> {
    pub fn new<'b>(cmds: &'b [Command]) -> CommandManager<'b> {
        match Self::try_new(cmds) {
            Ok(cm) => cm,
            Err(e) => panic!("{}", e),
        }
    }

    fn try_new<'b>(cmds: &'b [Command]) -> Result<CommandManager<'b>, String> {
        try!(Self::validate_cmds(cmds));
        Ok(CommandManager { cmds: cmds })
    }

    fn validate_cmds(cmds: &[Command]) -> Result<(), String> {
        for cmd in cmds {
            try!(Self::validate_params(cmd));
        }

        Ok(())
    }

    fn validate_params(cmd: &Command) -> Result<(), String> {
        let dynamic_param_count = cmd.params.iter().filter(|p| p.repeating || !p.required).count();
        if dynamic_param_count > 1 {
            return Err(format!("command-cli panic: Command '{}' cannot have more than one repeating or optional parameter.", cmd.name));
        }

        Ok(())
    }
}

/// Describes a command along with how to execute it and display help info for it.
pub struct Command {
    /// The name of the command.
    pub name: &'static str,

    /// A one-line description of what the command does.
    pub short_desc: &'static str,

    /// A description of the parameters the command takes.
    pub params: &'static [Parameter],

    /// A function which, given the command arguments and i/o handles, executes the command.
    pub handler: fn(&Arguments, &mut StreamProvider) -> CommandResult,
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
use self::CommandResult::*;

/// Describes a command parameter and how to display help info for it.
#[derive(Eq, PartialEq, Hash)]
pub struct Parameter {
    pub name: &'static str,
    pub required: bool,
    pub repeating: bool,
}

/// Describes the arguments to a command.
pub struct Arguments<'a> {
    /// A mapping from `Parameter` to the associated arguments for that parameter.
    param_to_args: HashMap<&'a Parameter, Vec<String>>,
}

impl<'a> Arguments<'a> {
    pub fn for_param(&self, param: &'a Parameter) -> &Vec<String> {
        &self.param_to_args[param]
    }

    /// Constructs a new `Arguments`, yielding `None` if the arguments do not
    /// match the provided parameter specification.
    fn new<'b>(params: &[&'b Parameter], args: Vec<String>) -> Option<Arguments<'b>> {
        let mut param_to_args = HashMap::new();
        let mut min_remaining = params.iter().filter(|p| p.required).count();
        let mut remaining = args.len();
        let mut args_iter = args.into_iter();

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

            param_to_args.insert(*param, param_args);
        }

        Some(Arguments { param_to_args: param_to_args })
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    #[test]
    fn arguments__new__too_few_args__returns_none() {
        let param = Parameter { name: "name", required: true, repeating: false };
        let args = Vec::new();

        let result = Arguments::new(&[&param], args);

        assert!(result.is_none());
    }

    #[test]
    fn arguments__new__optional_param_and_no_args__returns_empty() {
        let param = Parameter { name: "name", required: false, repeating: false };
        let args = Vec::new();

        let arguments = Arguments::new(&[&param], args).unwrap();

        assert_eq!(0, arguments.for_param(&param).len());
    }

    #[test]
    fn arguments__new__required__success() {
        let param1 = Parameter { name: "name1", required: true, repeating: false };
        let param2 = Parameter { name: "name2", required: true, repeating: false };
        let (arg1, arg2) = ("arg1".to_string(), "arg2".to_string());
        let args = vec![arg1.clone(), arg2.clone()];

        let arguments = Arguments::new(&[&param1, &param2], args).unwrap();

        assert_eq!(&vec![arg1], arguments.for_param(&param1));
        assert_eq!(&vec![arg2], arguments.for_param(&param2));
    }

    #[test]
    fn arguments__new__repeating_param_and_args__success() {
        let param = Parameter { name: "name", required: true, repeating: true };
        let args = vec!["arg1".to_string(), "arg2".to_string()];

        let arguments = Arguments::new(&[&param], args.clone()).unwrap();

        assert_eq!(&args, arguments.for_param(&param));
    }

    #[test]
    fn arguments__new__repeating_then_required__success() {
        let param1 = Parameter { name: "name1", required: true, repeating: true };
        let param2 = Parameter { name: "name2", required: true, repeating: false };
        let (arg1, arg2, arg3) = ("arg1".to_string(), "arg2".to_string(), "arg3".to_string());
        let args = vec![arg1.clone(), arg2.clone(), arg3.clone()];

        let arguments = Arguments::new(&[&param1, &param2], args).unwrap();

        assert_eq!(&vec![arg1, arg2], arguments.for_param(&param1));
        assert_eq!(&vec![arg3], arguments.for_param(&param2));
    }

    #[test]
    fn arguments__new__required_then_repeating__success() {
        let param1 = Parameter { name: "name", required: true, repeating: false };
        let param2 = Parameter { name: "name", required: true, repeating: true };
        let (arg1, arg2, arg3) = ("arg1".to_string(), "arg2".to_string(), "arg3".to_string());
        let args = vec![arg1.clone(), arg2.clone(), arg3.clone()];

        let arguments = Arguments::new(&[&param1, &param2], args).unwrap();

        assert_eq!(&vec![arg1], arguments.for_param(&param1));
        assert_eq!(&vec![arg2, arg3], arguments.for_param(&param2));
    }

    #[test]
    fn arguments__new__optional_then_required_with_one_arg__success() {
        let param1 = Parameter { name: "name1", required: false, repeating: false };
        let param2 = Parameter { name: "name2", required: true, repeating: false };
        let args = vec!["arg1".to_string()];

        let arguments = Arguments::new(&[&param1, &param2], args.clone()).unwrap();

        assert_eq!(0, arguments.for_param(&param1).len());
        assert_eq!(&args, arguments.for_param(&param2));
    }

    #[test]
    fn arguments__new__optional_then_required_with_two_args__success() {
        let param1 = Parameter { name: "name1", required: false, repeating: false };
        let param2 = Parameter { name: "name2", required: true, repeating: false };
        let (arg1, arg2) = ("arg1".to_string(), "arg2".to_string());
        let args = vec![arg1.clone(), arg2.clone()];

        let arguments = Arguments::new(&[&param1, &param2], args).unwrap();

        assert_eq!(&vec![arg1], arguments.for_param(&param1));
        assert_eq!(&vec![arg2], arguments.for_param(&param2));
    }

    #[test]
    fn arguments__new__required_then_optional_with_one_arg__success() {
        let param1 = Parameter { name: "name1", required: true, repeating: false };
        let param2 = Parameter { name: "name2", required: false, repeating: false };
        let args = vec!["arg1".to_string()];

        let arguments = Arguments::new(&[&param1, &param2], args.clone()).unwrap();

        assert_eq!(&args, arguments.for_param(&param1));
        assert_eq!(0, arguments.for_param(&param2).len());
    }

    #[test]
    fn arguments__new__required_then_optional_with_two_args__success() {
        let param1 = Parameter { name: "name1", required: true, repeating: false };
        let param2 = Parameter { name: "name2", required: false, repeating: false };
        let (arg1, arg2) = ("arg1".to_string(), "arg2".to_string());
        let args = vec![arg1.clone(), arg2.clone()];

        let arguments = Arguments::new(&[&param1, &param2], args).unwrap();

        assert_eq!(&vec![arg1], arguments.for_param(&param1));
        assert_eq!(&vec![arg2], arguments.for_param(&param2));
    }
}
