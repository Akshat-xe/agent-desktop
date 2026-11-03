use agent_desktop_core::{
    AppError, PlatformAdapter, commands::sandbox, commands::sandbox_action::SandboxAction,
};
use serde_json::Value;
use std::io::Write;
use std::process::ExitCode;

use crate::cli_args::sandbox::{SandboxActionCli, SandboxArgs, SandboxExecArgs};

pub(crate) fn dispatch(
    args: SandboxArgs,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AppError> {
    sandbox::execute(to_core(args.action), adapter)
}

pub(crate) fn run_exec(
    args: &SandboxExecArgs,
    cmd_name: &str,
    adapter: &dyn PlatformAdapter,
) -> ExitCode {
    let result = agent_desktop_core::validate_state_root_env()
        .map_err(crate::pre_dispatch_error)
        .and_then(|()| sandbox::execute_exec(args.name.clone(), args.args.clone(), adapter));
    match result {
        Ok(passthrough) => match write_stdout(&passthrough.stdout) {
            Ok(()) => exit_code(passthrough.status),
            Err(error) => crate::report_output_failure(error),
        },
        Err(error) => crate::finish(cmd_name, Err(error)),
    }
}

fn write_stdout(bytes: &[u8]) -> std::io::Result<()> {
    let stdout = std::io::stdout();
    let mut writer = std::io::BufWriter::new(stdout.lock());
    writer.write_all(bytes)?;
    writer.flush()
}

fn exit_code(status: i32) -> ExitCode {
    match u8::try_from(status) {
        Ok(code) => ExitCode::from(code),
        Err(_) => ExitCode::FAILURE,
    }
}

fn to_core(action: SandboxActionCli) -> SandboxAction {
    match action {
        SandboxActionCli::Create(create) => SandboxAction::Create {
            name: create.name,
            provider: create.provider,
            set: create.set,
        },
        SandboxActionCli::Start(args) => SandboxAction::Start { name: args.name },
        SandboxActionCli::Stop(args) => SandboxAction::Stop { name: args.name },
        SandboxActionCli::Status(args) => SandboxAction::Status { name: args.name },
        SandboxActionCli::List => SandboxAction::List,
        SandboxActionCli::Rm(args) => SandboxAction::Rm { name: args.name },
        SandboxActionCli::Exec(exec) => SandboxAction::Exec {
            name: exec.name,
            args: exec.args,
        },
    }
}

#[cfg(test)]
#[path = "sandbox_tests.rs"]
mod tests;
