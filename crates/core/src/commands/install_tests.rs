use super::execute;
use crate::refs::refs_test_support::HomeGuard;
use crate::sandbox::provider_test_support::ScriptedAdapter;
use crate::{AdapterError, ErrorCode};

const CHECK_READY: &str = r#"{"ok":true,"state":"ready","message":"The stub provider is ready.","next":"Run `agent-desktop install stub` to see it again."}"#;
const INSTALLED: &str = r#"{"ok":true,"state":"ready","message":"The stub provider is installed.","data":{"size_bytes":42}}"#;

#[test]
fn listing_without_a_provider_reports_the_stub_provider_state() {
    let _home = HomeGuard::new();
    let adapter = ScriptedAdapter::new().reply("check", CHECK_READY, 0);

    let data = execute(None, &adapter).expect("check replies");

    assert_eq!(data["providers"][0]["name"], "stub");
    assert_eq!(data["providers"][0]["state"], "ready");
    assert_eq!(
        data["providers"][0]["message"],
        "The stub provider is ready."
    );
    assert_eq!(
        data["providers"][0]["next"],
        "Run `agent-desktop install stub` to see it again."
    );
    let requests = adapter.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].args, vec!["check".to_string()]);
}

#[test]
fn a_provider_whose_check_fails_is_listed_with_its_error_message() {
    let _home = HomeGuard::new();
    let adapter = ScriptedAdapter::new().fail(
        "check",
        AdapterError::new(
            ErrorCode::PlatformNotSupported,
            "The provider is not installed.",
        )
        .with_suggestion("Install the provider, then run `agent-desktop install stub` again."),
    );

    let data = execute(None, &adapter).expect("one failing check does not fail the command");

    assert_eq!(data["providers"][0]["name"], "stub");
    assert_eq!(data["providers"][0]["state"], "unavailable");
    assert_eq!(
        data["providers"][0]["message"],
        "The provider is not installed."
    );
    assert_eq!(
        data["providers"][0]["next"],
        "Install the provider, then run `agent-desktop install stub` again."
    );
}

#[test]
fn installing_a_provider_returns_its_reply() {
    let _home = HomeGuard::new();
    let adapter = ScriptedAdapter::new().reply("install", INSTALLED, 0);

    let data = execute(Some("stub".to_string()), &adapter).expect("install replies");

    assert_eq!(data["name"], "stub");
    assert_eq!(data["state"], "ready");
    assert_eq!(data["message"], "The stub provider is installed.");
    assert_eq!(data["size_bytes"], 42);
    assert!(data.get("next").is_none());
    let requests = adapter.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].args, vec!["install".to_string()]);
    assert!(
        requests[0].script.ends_with("provider.sh"),
        "{}",
        requests[0].script.display()
    );
}

#[test]
fn an_unknown_provider_is_refused_before_any_script_runs() {
    let _home = HomeGuard::new();
    let adapter = ScriptedAdapter::new();

    let error = execute(Some("nomad".to_string()), &adapter).expect_err("unknown provider");

    assert_eq!(error.code(), "INVALID_ARGS");
    assert!(error.to_string().contains("stub"));
    assert!(adapter.requests().is_empty());
}
