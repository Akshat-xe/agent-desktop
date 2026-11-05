use super::*;
use std::os::unix::ffi::OsStrExt;

fn procargs(executable: &str, arguments: &[&str]) -> Vec<u8> {
    let mut buffer = (arguments.len() as i32).to_ne_bytes().to_vec();
    buffer.extend_from_slice(executable.as_bytes());
    buffer.extend_from_slice(b"\0\0\0");
    for argument in arguments {
        buffer.extend_from_slice(argument.as_bytes());
        buffer.push(0);
    }
    buffer.extend_from_slice(b"HOME=/Users/a\0");
    buffer
}

#[test]
fn first_argument_is_the_launch_command_not_the_executable_path() {
    let buffer = procargs(
        "/Applications/Foo.app/Contents/MacOS/Foo",
        &["com.example.renamed", "--flag"],
    );

    assert_eq!(first_argument(&buffer), Some(&b"com.example.renamed"[..]));
}

#[test]
fn first_argument_keeps_spaces_in_the_command() {
    let buffer = procargs(
        "/Applications/Foo Bar.app/Contents/MacOS/Foo Bar",
        &["/Applications/Foo Bar.app/Contents/MacOS/Foo Bar"],
    );

    assert_eq!(
        first_argument(&buffer),
        Some(&b"/Applications/Foo Bar.app/Contents/MacOS/Foo Bar"[..])
    );
}

#[test]
fn malformed_argument_blocks_have_no_command() {
    assert_eq!(first_argument(&[1, 0]), None);
    for count in [0_i32, -1] {
        let mut buffer = procargs("/bin/zsh", &["-zsh"]);
        buffer[..4].copy_from_slice(&count.to_ne_bytes());
        assert_eq!(first_argument(&buffer), None);
    }
    let mut unterminated = 1_i32.to_ne_bytes().to_vec();
    unterminated.extend_from_slice(b"/bin/zsh\0\0-zsh");
    assert_eq!(first_argument(&unterminated), None);
    let mut padding_only = 1_i32.to_ne_bytes().to_vec();
    padding_only.extend_from_slice(b"/bin/zsh\0\0\0");
    assert_eq!(first_argument(&padding_only), None);
}

#[test]
fn failed_process_listing_is_not_an_empty_inventory() {
    let error = require_pids(None).unwrap_err();

    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    assert!(error.is_explicitly_retryable());
    assert_eq!(error.details.unwrap()["source"], "libproc");
}

#[test]
fn current_process_command_matches_its_first_argument() {
    let own_pid = std::process::id() as libc::pid_t;
    let expected = std::env::args_os().next().unwrap();
    let mut seen = None;

    visit_commands_until(
        Instant::now() + std::time::Duration::from_secs(5),
        |pid, command| {
            if pid == own_pid {
                seen = Some(command.to_vec());
            }
            Ok(())
        },
    )
    .unwrap();

    assert_eq!(seen.as_deref(), Some(expected.as_bytes()));
}

#[test]
fn expired_deadline_stops_the_process_walk() {
    let mut visited = 0;
    let error = visit_commands_until(Instant::now(), |_, _| {
        visited += 1;
        Ok(())
    })
    .unwrap_err();

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(visited, 0);
}

#[test]
fn other_users_processes_have_no_readable_arguments() {
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let mut buffer = argument_buffer().unwrap();

    assert!(read_arguments(1, &mut buffer).is_none());
}
