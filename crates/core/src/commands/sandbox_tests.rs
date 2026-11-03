use super::{execute, execute_exec};
use crate::commands::sandbox_action::SandboxAction;
use crate::refs::refs_test_support::HomeGuard;
use crate::sandbox::provider_test_support::ScriptedAdapter;
use crate::sandbox::sandbox_store;
use crate::{AdapterError, AppError, DeliverySemantics, ErrorCode, ProviderScriptRequest};
use std::path::PathBuf;
use std::time::Duration;

const CREATED: &str = r#"{"ok":true,"state":"stopped","message":"Sandbox dev-1 is created.","next":"Run `agent-desktop sandbox start dev-1`."}"#;
const STOPPED: &str = r#"{"ok":true,"state":"stopped","message":"Sandbox dev-1 is stopped."}"#;
const RUNNING: &str = r#"{"ok":true,"state":"running","message":"Sandbox dev-1 is running."}"#;
const REMOVED: &str = r#"{"ok":true,"state":"stopped","message":"Sandbox dev-1 is removed."}"#;
const UNAVAILABLE: &str =
    r#"{"ok":true,"state":"unavailable","message":"This Mac cannot run virtual machines."}"#;

fn create_action() -> SandboxAction {
    SandboxAction::Create {
        name: "dev-1".to_string(),
        provider: "stub".to_string(),
        set: Vec::new(),
    }
}

fn state_root(home: &HomeGuard) -> PathBuf {
    home.path().join(".agent-desktop")
}

fn sandboxes_root(home: &HomeGuard) -> PathBuf {
    state_root(home).join("sandboxes")
}

fn env_value(request: &ProviderScriptRequest, key: &str) -> Option<String> {
    request
        .env
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.clone())
}

#[test]
fn creating_a_name_twice_names_the_existing_sandbox() {
    let _home = HomeGuard::new();
    let adapter = ScriptedAdapter::new().reply("create", CREATED, 0);
    execute(create_action(), &adapter).expect("first create");

    let error = execute(create_action(), &adapter).expect_err("second create is refused");

    assert_eq!(error.code(), "INVALID_ARGS");
    assert!(
        error.to_string().contains("dev-1"),
        "the message names the sandbox: {error}"
    );
    assert!(
        error.to_string().contains("stub"),
        "the message names the existing provider: {error}"
    );
    assert_eq!(
        adapter.requests().len(),
        1,
        "the second create runs nothing"
    );
}

#[test]
fn create_runs_the_create_verb_with_the_sandbox_environment() {
    let home = HomeGuard::new();
    let adapter = ScriptedAdapter::new().reply("create", CREATED, 0);

    execute(
        SandboxAction::Create {
            name: "dev-1".to_string(),
            provider: "stub".to_string(),
            set: vec!["binary=/tmp/agent-desktop".to_string()],
        },
        &adapter,
    )
    .expect("create replies");

    let requests = adapter.requests();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    let root = state_root(&home);
    assert_eq!(
        request.args,
        vec!["create".to_string(), "dev-1".to_string()]
    );
    assert!(
        request
            .script
            .starts_with(root.join("providers").join("stub")),
        "{}",
        request.script.display()
    );
    assert!(
        request.script.ends_with("provider.sh"),
        "{}",
        request.script.display()
    );
    assert_eq!(request.working_dir, root.join("sandboxes").join("dev-1"));
    assert_eq!(request.timeout, Duration::from_secs(60));
    assert_eq!(
        env_value(request, "AD_SANDBOX_NAME"),
        Some("dev-1".to_string())
    );
    assert_eq!(
        env_value(request, "AD_SANDBOX_DIR"),
        Some(root.join("sandboxes").join("dev-1").display().to_string())
    );
    assert_eq!(
        env_value(request, "AD_PROVIDER_DIR"),
        request.script.parent().map(|dir| dir.display().to_string())
    );
    assert_eq!(
        env_value(request, "AD_VERSION"),
        Some(env!("CARGO_PKG_VERSION").to_string())
    );
    assert_eq!(
        env_value(request, "AD_BINARY"),
        std::env::current_exe()
            .ok()
            .map(|path| path.display().to_string())
    );
    assert_eq!(
        env_value(request, "AD_SET_BINARY"),
        Some("/tmp/agent-desktop".to_string())
    );
}

#[test]
fn a_script_timeout_names_status_as_the_next_command() {
    let _home = HomeGuard::new();
    let adapter = ScriptedAdapter::new().fail(
        "create",
        AdapterError::new(ErrorCode::Timeout, "The provider script timed out."),
    );

    let error = execute(create_action(), &adapter).expect_err("the timeout surfaces");

    assert_eq!(error.code(), "TIMEOUT");
    assert!(
        error
            .suggestion()
            .is_some_and(|suggestion| suggestion.contains("sandbox status dev-1")),
        "{:?}",
        error.suggestion()
    );
    let AppError::Adapter(adapter_error) = &error else {
        panic!("the timeout keeps its adapter error");
    };
    assert_eq!(adapter_error.disposition, DeliverySemantics::unknown());
}

#[test]
fn a_failed_create_that_left_nothing_behind_removes_the_record() {
    let home = HomeGuard::new();
    let adapter = ScriptedAdapter::new();

    execute(create_action(), &adapter).expect_err("the provider create fails");

    assert!(
        !sandboxes_root(&home).join("dev-1").exists(),
        "a failed create leaves no sandbox behind"
    );
}

#[test]
fn a_failed_create_that_left_provider_files_keeps_the_record() {
    let home = HomeGuard::new();
    let sandbox_dir = sandboxes_root(&home).join("dev-1");
    crate::storage::private_file_parent::ensure_private(&sandbox_dir)
        .expect("private sandbox directory");
    std::fs::write(sandbox_dir.join("id_ed25519"), b"key").expect("provider file");
    let adapter = ScriptedAdapter::new();

    execute(create_action(), &adapter).expect_err("the provider create fails");

    assert!(
        sandbox_store::load(&sandboxes_root(&home), "dev-1").is_ok(),
        "the record stays so status can report what the provider left"
    );
}

#[test]
fn create_on_a_provider_that_cannot_run_here_is_refused_and_removes_the_record() {
    let home = HomeGuard::new();
    let adapter = ScriptedAdapter::new().reply("create", UNAVAILABLE, 0);

    let error = execute(create_action(), &adapter).expect_err("the provider is unavailable");

    assert_eq!(error.code(), "PLATFORM_NOT_SUPPORTED");
    assert_eq!(error.to_string(), "This Mac cannot run virtual machines.");
    assert_eq!(
        error.suggestion(),
        Some("Run `agent-desktop install` to see what each provider needs.")
    );
    assert!(
        !sandboxes_root(&home).join("dev-1").exists(),
        "an unavailable provider leaves no sandbox behind"
    );
}

#[test]
fn status_of_a_missing_sandbox_is_sandbox_not_found() {
    let _home = HomeGuard::new();
    let adapter = ScriptedAdapter::new();

    let error = execute(
        SandboxAction::Status {
            name: "ghost".to_string(),
        },
        &adapter,
    )
    .expect_err("no such sandbox");

    assert_eq!(error.code(), "SANDBOX_NOT_FOUND");
    assert!(adapter.requests().is_empty());
}

#[test]
fn removing_a_running_sandbox_is_refused_and_keeps_the_record() {
    let home = HomeGuard::new();
    let adapter = ScriptedAdapter::new()
        .reply("create", CREATED, 0)
        .reply("status", RUNNING, 0);
    execute(create_action(), &adapter).expect("create");

    let error = execute(
        SandboxAction::Rm {
            name: "dev-1".to_string(),
        },
        &adapter,
    )
    .expect_err("a running sandbox is refused");

    assert_eq!(error.code(), "INVALID_ARGS");
    assert_eq!(
        error.to_string(),
        "Sandbox dev-1 is running, so it was not removed."
    );
    assert_eq!(
        error.suggestion(),
        Some("Run `agent-desktop sandbox stop dev-1` first.")
    );
    assert_eq!(adapter.requests().len(), 2, "the rm verb never runs");
    assert!(
        sandbox_store::load(&sandboxes_root(&home), "dev-1").is_ok(),
        "the record stays"
    );
}

#[test]
fn removing_a_stopped_sandbox_removes_the_record() {
    let home = HomeGuard::new();
    let adapter = ScriptedAdapter::new()
        .reply("create", CREATED, 0)
        .reply("status", STOPPED, 0)
        .reply("rm", REMOVED, 0);
    execute(create_action(), &adapter).expect("create");

    let data = execute(
        SandboxAction::Rm {
            name: "dev-1".to_string(),
        },
        &adapter,
    )
    .expect("a stopped sandbox is removed");

    assert_eq!(data["name"], "dev-1");
    assert_eq!(data["provider"], "stub");
    assert_eq!(data["message"], "Sandbox dev-1 is removed.");
    assert_eq!(
        sandbox_store::load(&sandboxes_root(&home), "dev-1")
            .expect_err("the record is gone")
            .code(),
        "SANDBOX_NOT_FOUND"
    );
}

#[test]
fn exec_passes_the_guest_stdout_and_status_through() {
    let _home = HomeGuard::new();
    let guest = r#"{"version":"2.5","ok":false,"command":"snapshot","error":{"code":"STALE_REF"}}"#;
    let adapter = ScriptedAdapter::new()
        .reply("create", CREATED, 0)
        .reply("exec", guest, 1);
    execute(create_action(), &adapter).expect("create");

    let outcome = execute_exec(
        "dev-1".to_string(),
        vec![
            "snapshot".to_string(),
            "--app".to_string(),
            "Finder".to_string(),
        ],
        &adapter,
    )
    .expect("exec runs");

    assert_eq!(outcome.status, 1);
    assert_eq!(String::from_utf8_lossy(&outcome.stdout), guest);
    let requests = adapter.requests();
    assert_eq!(
        requests[1].args,
        vec!["exec", "dev-1", "--", "snapshot", "--app", "Finder"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<String>>()
    );
    assert_eq!(requests[1].timeout, Duration::from_secs(300));
}

#[test]
fn list_shows_a_sandbox_whose_status_fails_as_broken() {
    let _home = HomeGuard::new();
    let adapter = ScriptedAdapter::new().reply("create", CREATED, 0).fail(
        "status",
        AdapterError::new(
            ErrorCode::ActionFailed,
            "The stub provider lost its record.",
        ),
    );
    execute(create_action(), &adapter).expect("create");

    let data = execute(SandboxAction::List, &adapter).expect("list");

    assert_eq!(data["sandboxes"][0]["name"], "dev-1");
    assert_eq!(data["sandboxes"][0]["provider"], "stub");
    assert_eq!(data["sandboxes"][0]["state"], "broken");
    assert_eq!(
        data["sandboxes"][0]["message"],
        "The stub provider lost its record."
    );
    assert!(
        data["sandboxes"][0]["next"]
            .as_str()
            .is_some_and(|next| next.contains("sandbox rm dev-1")),
        "{}",
        data["sandboxes"][0]["next"]
    );
}
