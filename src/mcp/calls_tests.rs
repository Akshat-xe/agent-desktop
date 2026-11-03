use super::*;
use serde_json::json;

fn unspawnable() -> Command {
    Command::new("agent-desktop-calls-test-binary-that-does-not-exist")
}

#[test]
fn an_id_is_admitted_once_until_its_call_finishes() {
    let calls = Calls::default();
    assert!(calls.admit(&json!(1)));
    assert!(!calls.admit(&json!(1)));
    assert!(calls.admit(&json!("1")));
    assert!(calls.finish(&json!(1)));
    assert!(calls.admit(&json!(1)));
}

#[test]
fn a_call_cancelled_before_it_starts_never_spawns_and_gets_no_response() {
    let calls = Calls::default();
    assert!(calls.admit(&json!(7)));
    calls.cancel(&json!(7));
    assert!(calls.start(&json!(7), &mut unspawnable()).is_none());
    assert!(!calls.finish(&json!(7)));
}
