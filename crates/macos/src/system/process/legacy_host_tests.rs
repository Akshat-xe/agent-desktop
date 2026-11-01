use super::environment_has_marker;

fn procargs(arguments: &[&str], environment: &[&str]) -> Vec<u8> {
    let mut buffer = (arguments.len() as i32).to_ne_bytes().to_vec();
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
}

#[test]
fn empty_arguments_are_counted_before_reading_the_environment() {
    let marker = "AGENT_DESKTOP_INTERNAL_COMMAND_HOST=1";
    assert!(environment_has_marker(&procargs(
        &["agent-desktop", "", "wait", ""],
        &[marker]
    )));
    assert!(!environment_has_marker(&procargs(
        &["agent-desktop", "", marker],
        &["HOME=/Users/a"]
    )));
}

#[test]
fn only_an_exact_complete_environment_entry_matches() {
    for entry in [
        "AGENT_DESKTOP_INTERNAL_COMMAND_HOST=0",
        "AGENT_DESKTOP_INTERNAL_COMMAND_HOST=10",
        "AGENT_DESKTOP_INTERNAL_COMMAND_HOST=1suffix",
        "OTHER=AGENT_DESKTOP_INTERNAL_COMMAND_HOST=1",
        "PREFIX_AGENT_DESKTOP_INTERNAL_COMMAND_HOST=1",
        "AGENT_DESKTOP_INTERNAL_COMMAND_HOST",
    ] {
        assert!(!environment_has_marker(&procargs(
            &["agent-desktop"],
            &[entry]
        )));
    }
    let marker = "AGENT_DESKTOP_INTERNAL_COMMAND_HOST=1";
    let mut truncated = procargs(&["agent-desktop"], &[marker]);
    truncated.pop();
    assert!(!environment_has_marker(&truncated));
    assert!(!environment_has_marker(&procargs(
        &["agent-desktop"],
        &["", marker]
    )));
}

#[test]
fn malformed_argument_blocks_fail_closed() {
    assert!(!environment_has_marker(&[1, 0]));
    assert!(!environment_has_marker(&1i32.to_ne_bytes()));
    let marker = "AGENT_DESKTOP_INTERNAL_COMMAND_HOST=1";
    for count in [0, -1, i32::MAX] {
        let mut buffer = procargs(&["agent-desktop"], &[marker]);
        buffer[..4].copy_from_slice(&count.to_ne_bytes());
        assert!(!environment_has_marker(&buffer));
    }
    let mut missing_path_terminator = 1i32.to_ne_bytes().to_vec();
    missing_path_terminator.extend_from_slice(marker.as_bytes());
    assert!(!environment_has_marker(&missing_path_terminator));
    let mut marker_in_path = 1i32.to_ne_bytes().to_vec();
    marker_in_path.extend_from_slice(marker.as_bytes());
    marker_in_path.extend_from_slice(b"\0\0agent-desktop\0HOME=/Users/a\0");
    assert!(!environment_has_marker(&marker_in_path));
    let mut missing_argument_terminator = procargs(&["agent-desktop"], &[]);
    missing_argument_terminator.pop();
    assert!(!environment_has_marker(&missing_argument_terminator));
}
