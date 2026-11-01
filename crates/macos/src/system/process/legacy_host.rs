//! Stops command hosts that earlier agent-desktop versions left running.

use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::UnixStream;
use std::path::Path;

const HOST_MARKER: &[u8] = b"AGENT_DESKTOP_INTERNAL_COMMAND_HOST=1";
const MAX_ENTRIES: usize = 64;

unsafe extern "C" {
    fn proc_signal_with_audittoken(token: *mut [u32; 8], signal: libc::c_int) -> libc::c_int;
}

/// Stops every command host an earlier version left running for this user; it
/// changes no files and ignores failures so a command never waits on it.
pub fn stop_legacy_command_hosts() {
    let uid = unsafe { libc::geteuid() };
    let directory = std::env::temp_dir().join(format!("agent-desktop-host-{uid}"));
    let is_private = std::fs::symlink_metadata(&directory).is_ok_and(|metadata| {
        metadata.is_dir() && metadata.uid() == uid && metadata.mode() & 0o077 == 0
    });
    if !is_private {
        return;
    }
    let Ok(entries) = std::fs::read_dir(&directory) else {
        return;
    };
    entries
        .flatten()
        .take(MAX_ENTRIES)
        .map(|entry| entry.path())
        .filter(|path| is_legacy_socket_name(path))
        .for_each(|socket| stop_host_at(&socket, uid));
}

/// Whether a file has the name earlier versions gave a host socket: a 16-digit
/// hex stem with a `.sock` suffix.
pub(crate) fn is_legacy_socket_name(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".sock"))
        .is_some_and(|stem| stem.len() == 16 && stem.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

/// Whether an executable file name is one agent-desktop shipped: the CLI or the
/// npm package's platform build.
pub(crate) fn is_shipped_executable(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "agent-desktop" || name.starts_with("agent-desktop-darwin-"))
}

/// Whether a `KERN_PROCARGS2` buffer carries the host marker in its environment,
/// skipping the executable path and arguments that precede it.
pub(crate) fn environment_has_marker(procargs: &[u8]) -> bool {
    let Some((count, rest)) = procargs.split_first_chunk::<4>() else {
        return false;
    };
    let arguments = u32::from_ne_bytes(*count) as usize;
    let mut fields = rest
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty());
    fields
        .by_ref()
        .take(arguments.saturating_add(1))
        .for_each(drop);
    fields.any(|field| field == HOST_MARKER)
}

fn stop_host_at(socket: &Path, uid: u32) {
    let is_own_socket = std::fs::symlink_metadata(socket)
        .is_ok_and(|metadata| metadata.file_type().is_socket() && metadata.uid() == uid);
    if !is_own_socket {
        return;
    }
    let Ok(stream) = UnixStream::connect(socket) else {
        return;
    };
    let Some(mut token) = peer_token(&stream) else {
        return;
    };
    let pid = token[5] as libc::pid_t;
    let own = std::process::id() as libc::pid_t;
    if token[1] != uid || pid <= 0 || pid == own {
        return;
    }
    let is_host = process_arguments(pid).is_some_and(|args| environment_has_marker(&args))
        && executable_path(pid).is_some_and(|path| is_shipped_executable(&path));
    if is_host {
        unsafe { proc_signal_with_audittoken(&mut token, libc::SIGTERM) };
    }
}

fn peer_token(stream: &UnixStream) -> Option<[u32; 8]> {
    let mut token = [0u32; 8];
    let mut length = std::mem::size_of::<[u32; 8]>() as libc::socklen_t;
    let read = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_LOCAL,
            libc::LOCAL_PEERTOKEN,
            token.as_mut_ptr().cast(),
            &mut length,
        )
    };
    (read == 0).then_some(token)
}

fn process_arguments(pid: libc::pid_t) -> Option<Vec<u8>> {
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
    let mut size: libc::size_t = 0;
    let null = std::ptr::null_mut();
    if unsafe { libc::sysctl(mib.as_mut_ptr(), 3, null, &mut size, null, 0) } != 0 {
        return None;
    }
    let mut buffer = vec![0u8; size];
    let read = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            3,
            buffer.as_mut_ptr().cast(),
            &mut size,
            null,
            0,
        )
    };
    (read == 0).then(|| {
        buffer.truncate(size);
        buffer
    })
}

fn executable_path(pid: libc::pid_t) -> Option<std::path::PathBuf> {
    let mut buffer = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    let written =
        unsafe { libc::proc_pidpath(pid, buffer.as_mut_ptr().cast(), buffer.len() as u32) };
    let written = usize::try_from(written)
        .ok()
        .filter(|written| *written > 0)?;
    buffer.truncate(written);
    String::from_utf8(buffer).ok().map(std::path::PathBuf::from)
}

#[cfg(test)]
#[path = "legacy_host_tests.rs"]
mod tests;
