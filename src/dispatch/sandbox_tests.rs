use agent_desktop_core::commands::sandbox_action::SandboxAction;
use agent_desktop_core::{AppError, ErrorCode};
use clap::Parser;

use super::{reject_outer_options, to_core};
use crate::cli_args::sandbox::{
    SandboxActionCli, SandboxCreateArgs, SandboxExecArgs, SandboxNameArgs,
};

fn named(name: &str) -> SandboxNameArgs {
    SandboxNameArgs {
        name: name.to_owned(),
    }
}

fn name_of(action: SandboxAction) -> String {
    match action {
        SandboxAction::Start { name }
        | SandboxAction::Stop { name }
        | SandboxAction::Status { name }
        | SandboxAction::Rm { name } => name,
        other => panic!("expected a named action, got {other:?}"),
    }
}

#[test]
fn every_cli_action_carries_its_fields_to_the_core_action() {
    let create = to_core(SandboxActionCli::Create(SandboxCreateArgs {
        name: "dev".into(),
        provider: "macos-vm".into(),
        set: vec!["from=base".into()],
    }));
    let SandboxAction::Create {
        name,
        provider,
        set,
    } = create
    else {
        panic!("expected create");
    };
    assert_eq!(name, "dev");
    assert_eq!(provider, "macos-vm");
    assert_eq!(set, ["from=base"]);

    for action in [
        SandboxActionCli::Start(named("dev")),
        SandboxActionCli::Stop(named("dev")),
        SandboxActionCli::Status(named("dev")),
        SandboxActionCli::Rm(named("dev")),
    ] {
        assert_eq!(name_of(to_core(action)), "dev");
    }

    assert!(matches!(
        to_core(SandboxActionCli::List),
        SandboxAction::List
    ));

    let exec = to_core(SandboxActionCli::Exec(SandboxExecArgs {
        name: "dev".into(),
        args: vec!["snapshot".into(), "--app".into(), "Finder".into()],
    }));
    let SandboxAction::Exec { name, args } = exec else {
        panic!("expected exec");
    };
    assert_eq!(name, "dev");
    assert_eq!(args, ["snapshot", "--app", "Finder"]);
}

#[test]
fn outer_global_options_are_rejected_before_sandbox_exec() {
    let plain =
        crate::Cli::try_parse_from(["agent-desktop", "sandbox", "exec", "dev", "--", "version"])
            .unwrap();
    assert!(reject_outer_options(&plain).is_ok());

    let outer = crate::Cli::try_parse_from([
        "agent-desktop",
        "--session",
        "abc",
        "sandbox",
        "exec",
        "dev",
        "--",
        "version",
    ])
    .unwrap();
    let AppError::Adapter(error) = reject_outer_options(&outer).unwrap_err() else {
        panic!("outer options must be an adapter error");
    };
    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert_eq!(
        error.message,
        "Options before `sandbox exec` are not applied inside the sandbox."
    );
    assert_eq!(
        error.suggestion.as_deref(),
        Some(
            "Put agent-desktop options after `--`, for example `agent-desktop sandbox exec dev -- snapshot --app Finder --session <id>`."
        )
    );
}
