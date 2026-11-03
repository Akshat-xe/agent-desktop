use serde_json::json;

use crate::sandbox::provider_protocol::{exec_outcome, parse_reply};
use crate::sandbox::provider_reply::ProviderReply;
use crate::sandbox::provider_script_output::ProviderScriptOutput;
use crate::sandbox::sandbox_state::SandboxState;
use crate::{AdapterError, AppError, DeliverySemantics, ErrorCode};

const READY_LINE: &str = r#"{"ok":true,"state":"ready","message":"The dev sandbox is ready.","next":"agent-desktop sandbox exec dev -- snapshot","data":{"changed":true}}"#;
const UNREADABLE: &str = "The macos-vm provider returned output agent-desktop could not read.";
const STATUS_SUGGESTION: &str = "Run `agent-desktop sandbox status dev` to see its current state.";
const NEXT_SUGGESTION: &str = "Run `agent-desktop sandbox list` to see your sandboxes.";
const LIST_SUGGESTION: &str =
    "Run `agent-desktop sandbox list` to find the sandbox and run the command again.";

fn output(stdout: &str, stderr: &str) -> ProviderScriptOutput {
    ProviderScriptOutput {
        status: Some(1),
        stdout: stdout.as_bytes().to_vec(),
        stderr_tail: stderr.to_owned(),
    }
}

fn failure(code: &str, message: &str, suggestion: &str) -> String {
    json!({
        "ok": false,
        "code": code,
        "message": message,
        "suggestion": suggestion,
    })
    .to_string()
}

fn adapter(error: AppError) -> AdapterError {
    match error {
        AppError::Adapter(error) => error,
        other => panic!("expected an adapter error, got {other:?}"),
    }
}

fn failed_reply(provider: &str, name: Option<&str>, mutating: bool, stdout: &str) -> AdapterError {
    adapter(parse_reply(provider, name, mutating, &output(stdout, "")).unwrap_err())
}

#[test]
fn a_ready_reply_parses_to_success_with_state_message_and_next() {
    let reply = parse_reply("macos-vm", Some("dev"), true, &output(READY_LINE, "")).unwrap();

    let ProviderReply {
        state,
        message,
        next,
        data,
    } = reply;

    assert_eq!(state, SandboxState::Ready);
    assert_eq!(message, "The dev sandbox is ready.");
    assert_eq!(
        next.as_deref(),
        Some("agent-desktop sandbox exec dev -- snapshot")
    );
    assert_eq!(data, Some(json!({ "changed": true })));
}

#[test]
fn the_last_non_empty_stdout_line_is_the_reply() {
    let stdout = format!("building the image\n{READY_LINE}\n\n");
    let reply = parse_reply("macos-vm", Some("dev"), false, &output(&stdout, "")).unwrap();

    let ProviderReply { state, next, .. } = reply;

    assert_eq!(state, SandboxState::Ready);
    assert!(next.is_some());
}

#[test]
fn an_unknown_code_becomes_action_failed_and_keeps_message_and_suggestion() {
    let stdout = failure(
        "BOGUS",
        "The macos-vm provider refused to create dev.",
        NEXT_SUGGESTION,
    );
    let error = failed_reply("macos-vm", Some("dev"), false, &stdout);

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.message,
        "The macos-vm provider refused to create dev."
    );
    assert_eq!(error.suggestion.as_deref(), Some(NEXT_SUGGESTION));
}

#[test]
fn the_six_codes_the_contract_allows_are_preserved() {
    let allowed = [
        ("INVALID_ARGS", ErrorCode::InvalidArgs),
        ("PLATFORM_NOT_SUPPORTED", ErrorCode::PlatformNotSupported),
        ("PERM_DENIED", ErrorCode::PermDenied),
        ("ACTION_FAILED", ErrorCode::ActionFailed),
        ("TIMEOUT", ErrorCode::Timeout),
        ("SANDBOX_NOT_FOUND", ErrorCode::SandboxNotFound),
    ];

    for (code, expected) in allowed {
        let stdout = failure(code, "The macos-vm provider stopped.", NEXT_SUGGESTION);
        let error = failed_reply("macos-vm", Some("dev"), false, &stdout);

        assert_eq!(error.code, expected, "{code}");
        assert_eq!(error.message, "The macos-vm provider stopped.");
    }
}

#[test]
fn a_non_json_last_line_is_unreadable_output_with_stderr_only_in_the_detail() {
    let stderr = "Error: image build failed\nexit status 1";
    let output = output("starting the build...\nnot json at all\n", stderr);
    let error = adapter(parse_reply("macos-vm", Some("dev"), false, &output).unwrap_err());

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.message, UNREADABLE);
    assert_eq!(error.suggestion.as_deref(), Some(STATUS_SUGGESTION));
    assert_eq!(error.platform_detail.as_deref(), Some(stderr));
    assert!(!error.message.contains("image build failed"));
}

#[test]
fn an_incomplete_reply_is_unreadable_output() {
    let broken = [
        r#"{"ok":true}"#,
        r#"{"ok":true,"state":"parked","message":"The dev sandbox is ready."}"#,
        r#"{"ok":false,"code":"ACTION_FAILED","message":"The macos-vm provider stopped."}"#,
        r#"{"state":"ready","message":"The dev sandbox is ready."}"#,
        "[1,2,3]",
    ];

    for stdout in broken {
        let error = failed_reply("macos-vm", Some("dev"), false, stdout);

        assert_eq!(error.code, ErrorCode::ActionFailed, "{stdout}");
        assert_eq!(error.message, UNREADABLE, "{stdout}");
        assert_eq!(
            error.suggestion.as_deref(),
            Some(STATUS_SUGGESTION),
            "{stdout}"
        );
    }
}

#[test]
fn a_long_stderr_tail_is_truncated_to_two_thousand_characters() {
    let empty = failed_reply("macos-vm", Some("dev"), false, "not json");
    assert!(empty.platform_detail.is_none());

    let stderr = "x".repeat(3000);
    let output = output("not json", &stderr);
    let error = adapter(parse_reply("macos-vm", Some("dev"), false, &output).unwrap_err());

    assert_eq!(error.platform_detail.expect("detail").len(), 2000);
}

#[test]
fn a_failed_mutating_verb_reports_unknown_delivery() {
    let stdout = failure(
        "ACTION_FAILED",
        "The macos-vm provider could not start dev.",
        NEXT_SUGGESTION,
    );
    let error = failed_reply("macos-vm", Some("dev"), true, &stdout);

    assert_eq!(error.disposition, DeliverySemantics::unknown());
}

#[test]
fn a_mutating_verb_that_reports_no_change_is_not_delivered() {
    let stdout = json!({
        "ok": false,
        "code": "ACTION_FAILED",
        "message": "The macos-vm provider could not start dev.",
        "suggestion": NEXT_SUGGESTION,
        "data": { "changed": false },
    })
    .to_string();
    let error = failed_reply("macos-vm", Some("dev"), true, &stdout);

    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn a_failed_read_verb_is_not_delivered() {
    let stdout = failure(
        "TIMEOUT",
        "The macos-vm provider did not answer.",
        NEXT_SUGGESTION,
    );
    let error = failed_reply("macos-vm", Some("dev"), false, &stdout);

    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn unreadable_output_from_a_mutating_verb_reports_unknown_delivery() {
    let error = failed_reply("macos-vm", Some("dev"), true, "");

    assert_eq!(error.disposition, DeliverySemantics::unknown());
}

#[test]
fn without_a_sandbox_name_the_next_step_is_the_install_command() {
    let error = failed_reply("macos-vm", None, false, "");

    assert_eq!(
        error.suggestion.as_deref(),
        Some("Run `agent-desktop install macos-vm` to see its current state.")
    );
}

#[test]
fn exec_passes_a_guest_envelope_through_byte_for_byte() {
    let mut guest =
        br#"{"version":"2.5","ok":false,"command":"snapshot","error":{"code":"STALE_REF"}}"#
            .to_vec();
    guest.push(b'\n');
    guest.push(0xff);
    let output = ProviderScriptOutput {
        status: Some(1),
        stdout: guest.clone(),
        stderr_tail: "the guest refused".to_owned(),
    };

    let passthrough = exec_outcome("macos-vm", &output).unwrap();

    assert_eq!(passthrough.stdout, guest);
    assert_eq!(passthrough.status, 1);
}

#[test]
fn exec_status_125_returns_the_provider_failure_object() {
    let stdout = failure(
        "SANDBOX_NOT_FOUND",
        "The sandbox dev does not exist.",
        NEXT_SUGGESTION,
    );
    let output = ProviderScriptOutput {
        status: Some(125),
        stdout: stdout.into_bytes(),
        stderr_tail: String::new(),
    };

    let error = adapter(exec_outcome("macos-vm", &output).unwrap_err());

    assert_eq!(error.code, ErrorCode::SandboxNotFound);
    assert_eq!(error.message, "The sandbox dev does not exist.");
    assert_eq!(error.suggestion.as_deref(), Some(NEXT_SUGGESTION));
}

#[test]
fn exec_status_125_with_unreadable_output_is_action_failed() {
    for stdout in ["not a failure object\n".to_owned(), READY_LINE.to_owned()] {
        let output = ProviderScriptOutput {
            status: Some(125),
            stdout: stdout.clone().into_bytes(),
            stderr_tail: "ssh: connect to host failed".to_owned(),
        };

        let error = adapter(exec_outcome("macos-vm", &output).unwrap_err());

        assert_eq!(error.code, ErrorCode::ActionFailed, "{stdout}");
        assert_eq!(error.message, UNREADABLE, "{stdout}");
        assert_eq!(error.suggestion.as_deref(), Some(LIST_SUGGESTION));
        assert_eq!(
            error.platform_detail.as_deref(),
            Some("ssh: connect to host failed")
        );
    }
}

#[test]
fn exec_without_an_exit_status_reports_a_plain_error() {
    let output = ProviderScriptOutput {
        status: None,
        stdout: Vec::<u8>::new(),
        stderr_tail: "ssh died".to_owned(),
    };

    let error = adapter(exec_outcome("macos-vm", &output).unwrap_err());

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.message,
        "The macos-vm provider stopped before agent-desktop could read its exit status."
    );
    assert_eq!(error.suggestion.as_deref(), Some(LIST_SUGGESTION));
    assert_eq!(error.platform_detail.as_deref(), Some("ssh died"));
}
