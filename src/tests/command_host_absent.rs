#![cfg(target_os = "macos")]

use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::os::unix::net::UnixListener;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

static HOST_TEST_LOCK: Mutex<()> = Mutex::new(());

struct ReapedChild(Option<Child>);

impl ReapedChild {
    fn child(&mut self) -> &mut Child {
        self.0.as_mut().expect("child is present")
    }
}

impl Drop for ReapedChild {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn private_dir(label: &str) -> PathBuf {
    let path = PathBuf::from(format!("/tmp/adh-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&path)
        .expect("create private dir");
    path
}

fn run(args: &[&str], tmpdir: &Path, home: &Path) -> Output {
    let mut child = ReapedChild(Some(
        Command::new(env!("CARGO_BIN_EXE_agent-desktop"))
            .args(args)
            .env("TMPDIR", tmpdir)
            .env("AGENT_DESKTOP_HOME", home)
            .env_remove("AGENT_DESKTOP_INTERNAL_COMMAND_HOST")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("binary starts"),
    ));
    let deadline = Instant::now() + Duration::from_secs(60);
    while child.child().try_wait().expect("poll child").is_none() {
        assert!(Instant::now() < deadline, "{args:?} did not finish in 60s");
        std::thread::sleep(Duration::from_millis(20));
    }
    child
        .0
        .take()
        .expect("child is present")
        .wait_with_output()
        .expect("collect output")
}

fn wait_process(tmpdir: &Path, home: &Path, marked: bool) -> ReapedChild {
    let mut command = Command::new(env!("CARGO_BIN_EXE_agent-desktop"));
    command
        .args(["wait", "10000"])
        .env("TMPDIR", tmpdir)
        .env("AGENT_DESKTOP_HOME", home)
        .env_remove("AGENT_DESKTOP_INTERNAL_COMMAND_HOST")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if marked {
        command.env("AGENT_DESKTOP_INTERNAL_COMMAND_HOST", "1");
    }
    ReapedChild(Some(command.spawn().expect("wait process starts")))
}

fn host_marked_processes(tmpdir: &Path) -> Vec<String> {
    let output = Command::new("ps")
        .args(["eww", "-A", "-o", "pid=,command="])
        .output()
        .expect("ps runs");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| {
            line.contains("AGENT_DESKTOP_INTERNAL_COMMAND_HOST")
                && line.contains(&tmpdir.display().to_string())
        })
        .map(str::to_owned)
        .collect()
}

#[test]
fn commands_never_reach_a_listening_command_host_socket() {
    let _lock = HOST_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let tmpdir = private_dir("armed");
    let home = private_dir("home");
    let uid = std::fs::metadata(&tmpdir).expect("stat tmpdir").uid();
    let host_dir = tmpdir.join(format!("agent-desktop-host-{uid}"));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&host_dir)
        .expect("create host dir");
    let listener =
        UnixListener::bind(host_dir.join("0123456789abcdef.sock")).expect("bind decoy socket");
    listener.set_nonblocking(true).expect("nonblocking");

    let version = run(&["version"], &tmpdir, &home);
    assert!(version.status.success(), "{:?}", version);
    run(&["list-apps"], &tmpdir, &home);
    let batch = run(&["batch", r#"[{"command":"version"}]"#], &tmpdir, &home);
    assert!(batch.status.success(), "{:?}", batch);

    assert!(
        matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock),
        "a command connected to the host socket"
    );
    assert!(
        host_dir.join("0123456789abcdef.sock").exists(),
        "a listener that is not an agent-desktop host must be left alone"
    );
    assert!(host_marked_processes(&tmpdir).is_empty());
    let _ = std::fs::remove_dir_all(&tmpdir);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn commands_do_not_create_a_host_directory_or_process() {
    let _lock = HOST_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let tmpdir = private_dir("fresh");
    let home = private_dir("freshhome");
    let uid = std::fs::metadata(&tmpdir).expect("stat tmpdir").uid();
    run(&["version"], &tmpdir, &home);
    run(&["list-apps"], &tmpdir, &home);
    run(&["batch", r#"[{"command":"version"}]"#], &tmpdir, &home);
    assert!(!tmpdir.join(format!("agent-desktop-host-{uid}")).exists());
    assert!(host_marked_processes(&tmpdir).is_empty());
    let _ = std::fs::remove_dir_all(&tmpdir);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn a_dead_legacy_socket_is_left_alone_and_does_not_block_commands() {
    let _lock = HOST_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let tmpdir = private_dir("stale");
    let home = private_dir("stalehome");
    let uid = std::fs::metadata(&tmpdir).expect("stat tmpdir").uid();
    let host_dir = tmpdir.join(format!("agent-desktop-host-{uid}"));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&host_dir)
        .expect("create host dir");
    let socket = host_dir.join("fedcba9876543210.sock");
    drop(UnixListener::bind(&socket).expect("bind stale socket"));
    let version = run(&["version"], &tmpdir, &home);
    assert!(version.status.success(), "{:?}", version);
    assert!(
        socket.exists(),
        "files in the legacy host directory are never deleted"
    );
    let _ = std::fs::remove_dir_all(&tmpdir);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn version_stops_a_marked_legacy_host_without_a_socket_in_another_tmpdir() {
    let _lock = HOST_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let tmpdir = private_dir("marker");
    let home = private_dir("markerhome");
    let command_tmpdir = private_dir("cleanup");
    let mut marked = wait_process(&tmpdir, &home, true);
    let deadline = Instant::now() + Duration::from_secs(3);
    while host_marked_processes(&tmpdir).is_empty() {
        assert!(
            marked
                .child()
                .try_wait()
                .expect("poll marked process")
                .is_none(),
            "marked stand-in exited before cleanup"
        );
        assert!(Instant::now() < deadline, "marked stand-in was not visible");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        marked
            .child()
            .try_wait()
            .expect("poll marked process")
            .is_none()
    );

    let deadline = Instant::now() + Duration::from_secs(3);
    let version = run(&["version"], &command_tmpdir, &home);
    assert!(version.status.success(), "{version:?}");
    let status = loop {
        assert!(
            Instant::now() < deadline,
            "marked stand-in survived cleanup for 3s"
        );
        if let Some(status) = marked.child().try_wait().expect("poll marked process") {
            break status;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(
        status.signal(),
        Some(15),
        "marked stand-in should exit on SIGTERM: {status}"
    );
    let _ = std::fs::remove_dir_all(&command_tmpdir);
    let _ = std::fs::remove_dir_all(&tmpdir);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn version_leaves_the_same_executable_running_without_the_host_marker() {
    let _lock = HOST_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let tmpdir = private_dir("unmarked");
    let home = private_dir("unmarkedhome");
    let mut unmarked = wait_process(&tmpdir, &home, false);
    assert!(
        unmarked
            .child()
            .try_wait()
            .expect("poll unmarked process")
            .is_none()
    );
    let version = run(&["version"], &tmpdir, &home);
    assert!(version.status.success(), "{version:?}");
    let deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < deadline {
        assert!(
            unmarked
                .child()
                .try_wait()
                .expect("poll unmarked process")
                .is_none(),
            "cleanup stopped an ordinary command without the host marker"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(unmarked);
    let _ = std::fs::remove_dir_all(&tmpdir);
    let _ = std::fs::remove_dir_all(&home);
}
