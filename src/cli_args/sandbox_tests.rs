use clap::Parser;

use super::SandboxActionCli;
use crate::cli::{Cli, Commands};

fn sandbox(argv: &[&str]) -> Commands {
    let mut full = vec!["agent-desktop", "sandbox"];
    full.extend_from_slice(argv);
    let cli = Cli::try_parse_from(full).expect("the sandbox command parses");
    cli.command.expect("the sandbox command is present")
}

fn exec_action(argv: &[&str]) -> super::SandboxExecArgs {
    let Commands::Sandbox(args) = sandbox(argv) else {
        panic!("expected the sandbox command");
    };
    let SandboxActionCli::Exec(exec) = args.action else {
        panic!("expected the exec action");
    };
    exec
}

#[test]
fn sandbox_exec_keeps_the_guest_arguments_byte_for_byte_after_the_separator() {
    let exec = exec_action(&["exec", "dev", "--", "snapshot", "--app", "Finder"]);
    assert_eq!(exec.name, "dev");
    assert_eq!(exec.args, ["snapshot", "--app", "Finder"]);

    let leading_hyphen = exec_action(&["exec", "dev", "--", "version", "--verbose"]);
    assert_eq!(leading_hyphen.args, ["version", "--verbose"]);
}

#[test]
fn sandbox_exec_requires_at_least_one_guest_argument() {
    let error = Cli::try_parse_from(["agent-desktop", "sandbox", "exec", "dev"])
        .expect_err("exec without guest arguments must not parse");

    assert_eq!(error.exit_code(), 2);
}

#[test]
fn sandbox_create_without_a_provider_exits_two_naming_the_flag() {
    let error = Cli::try_parse_from(["agent-desktop", "sandbox", "create", "dev"])
        .expect_err("a create without --provider must not parse");

    assert_eq!(error.exit_code(), 2);
    assert!(
        error.to_string().contains("--provider"),
        "the message must name the flag: {error}"
    );
}

#[test]
fn sandbox_create_collects_every_set_option() {
    let Commands::Sandbox(args) = sandbox(&[
        "create",
        "dev",
        "--provider",
        "macos-vm",
        "--set",
        "ipsw=/tmp/image.ipsw",
        "--set",
        "from=base",
    ]) else {
        panic!("expected the sandbox command");
    };
    let SandboxActionCli::Create(create) = args.action else {
        panic!("expected the create action");
    };

    assert_eq!(create.name, "dev");
    assert_eq!(create.provider, "macos-vm");
    assert_eq!(create.set, ["ipsw=/tmp/image.ipsw", "from=base"]);
}

#[test]
fn sandbox_list_and_status_are_the_only_read_only_sandbox_actions() {
    for (argv, mutating) in [
        (&["list"][..], false),
        (&["status", "dev"][..], false),
        (&["create", "dev", "--provider", "macos-vm"][..], true),
        (&["start", "dev"][..], true),
        (&["stop", "dev"][..], true),
        (&["rm", "dev"][..], true),
        (&["exec", "dev", "--", "version"][..], true),
    ] {
        assert_eq!(
            sandbox(argv).is_mutating(),
            mutating,
            "sandbox {argv:?} mutating classification"
        );
    }
}

#[test]
fn install_takes_an_optional_provider_and_counts_as_a_mutation() {
    let cli = Cli::try_parse_from(["agent-desktop", "install"]).expect("install parses alone");
    let command = cli.command.expect("the install command is present");
    assert_eq!(command.name(), "install");
    assert!(command.is_mutating());
    let Commands::Install(args) = command else {
        panic!("expected the install command");
    };
    assert!(args.provider.is_none());

    let cli =
        Cli::try_parse_from(["agent-desktop", "install", "macos-vm"]).expect("provider parses");
    let Commands::Install(args) = cli.command.expect("command") else {
        panic!("expected the install command");
    };
    assert_eq!(args.provider.as_deref(), Some("macos-vm"));
}
