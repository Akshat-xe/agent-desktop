use super::{environment_has_marker, is_legacy_socket_name, is_shipped_executable};
use std::path::Path;

#[test]
fn only_the_exact_legacy_socket_name_is_recognized() {
    assert!(is_legacy_socket_name(Path::new(
        "/tmp/agent-desktop-host-501/0123456789abcdef.sock"
    )));
    for other in [
        "decoy.sock",
        "0123456789abcdeg.sock",
        "0123456789abcdef.owner.lock",
        "0123456789abcdef",
    ] {
        assert!(!is_legacy_socket_name(Path::new(other)), "{other}");
    }
}

#[test]
fn only_shipped_executable_names_qualify() {
    assert!(is_shipped_executable(Path::new(
        "/usr/local/bin/agent-desktop"
    )));
    assert!(is_shipped_executable(Path::new(
        "/opt/npm/bin/agent-desktop-darwin-arm64"
    )));
    assert!(!is_shipped_executable(Path::new(
        "/tmp/agent-desktop-imposter"
    )));
    assert!(!is_shipped_executable(Path::new("/usr/bin/python3")));
}

fn procargs(arguments: &[&str], environment: &[&str]) -> Vec<u8> {
    let mut buffer = (arguments.len() as u32).to_ne_bytes().to_vec();
    buffer.extend_from_slice(b"/usr/local/bin/agent-desktop\0\0\0");
    for field in arguments.iter().chain(environment) {
        buffer.extend_from_slice(field.as_bytes());
        buffer.push(0);
    }
    buffer
}

#[test]
fn the_marker_counts_only_in_the_environment() {
    let marker = "AGENT_DESKTOP_INTERNAL_COMMAND_HOST=1";
    assert!(environment_has_marker(&procargs(
        &["agent-desktop"],
        &["HOME=/Users/a", marker]
    )));
    assert!(!environment_has_marker(&procargs(
        &["agent-desktop", marker],
        &["HOME=/Users/a"]
    )));
    assert!(!environment_has_marker(&[1, 0]));
}
