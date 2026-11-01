#![cfg(target_os = "macos")]

use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

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
    let mut child = Command::new(env!("CARGO_BIN_EXE_agent-desktop"))
        .args(args)
        .env("TMPDIR", tmpdir)
        .env("AGENT_DESKTOP_HOME", home)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("binary starts");
    let deadline = Instant::now() + Duration::from_secs(60);
    while child.try_wait().expect("poll child").is_none() {
        assert!(Instant::now() < deadline, "{args:?} did not finish in 60s");
        std::thread::sleep(Duration::from_millis(20));
    }
    child.wait_with_output().expect("collect output")
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
    let connections = Arc::new(AtomicUsize::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let watcher = {
        let (connections, stop) = (connections.clone(), stop.clone());
        std::thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                if let Ok((mut stream, _)) = listener.accept() {
                    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                    let mut buffer = [0u8; 64];
                    if let Ok(read) = std::io::Read::read(&mut stream, &mut buffer) {
                        connections.fetch_add(read, Ordering::SeqCst);
                    }
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        })
    };

    let version = run(&["version"], &tmpdir, &home);
    assert!(version.status.success(), "{:?}", version);
    run(&["list-apps"], &tmpdir, &home);
    let batch = run(&["batch", r#"[{"command":"version"}]"#], &tmpdir, &home);
    assert!(batch.status.success(), "{:?}", batch);

    std::thread::sleep(Duration::from_millis(300));
    stop.store(true, Ordering::SeqCst);
    watcher.join().expect("watcher joins");
    assert_eq!(
        connections.load(Ordering::SeqCst),
        0,
        "a command sent bytes to the host socket"
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
fn the_host_process_check_finds_a_process_carrying_the_marker() {
    let tmpdir = private_dir("marker");
    let home = private_dir("markerhome");
    let mut marked = Command::new(env!("CARGO_BIN_EXE_agent-desktop"))
        .args(["wait", "5000"])
        .env("AGENT_DESKTOP_INTERNAL_COMMAND_HOST", "1")
        .env("AGENT_DESKTOP_HOME", &home)
        .env("TMPDIR", &tmpdir)
        .stdout(std::process::Stdio::null())
        .spawn()
        .expect("marked process starts");
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut found = false;
    while Instant::now() < deadline && !found {
        found = !host_marked_processes(&tmpdir).is_empty();
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = marked.kill();
    let _ = marked.wait();
    let _ = std::fs::remove_dir_all(&tmpdir);
    assert!(
        found,
        "the marker check must see a process that carries the marker"
    );
}
