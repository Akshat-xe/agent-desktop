//! Stops command hosts that earlier agent-desktop versions left running.

use std::mem::{MaybeUninit, size_of};

const HOST_MARKER: &[u8] = b"AGENT_DESKTOP_INTERNAL_COMMAND_HOST=1";
const PROC_UID_ONLY: u32 = 4;

/// Sends SIGTERM to this user's processes whose environment carries the old command host marker.
/// A process whose owner, start time or environment cannot be read is left alone.
pub fn stop_legacy_command_hosts() {
    let uid = unsafe { libc::geteuid() };
    let own_pid = std::process::id() as libc::pid_t;
    for pid in user_processes(uid) {
        if pid <= 0 || pid == own_pid {
            continue;
        }
        let Some(started_at) = process_start_time(pid, uid) else {
            continue;
        };
        if process_arguments(pid).is_some_and(|args| environment_has_marker(&args))
            && process_start_time(pid, uid) == Some(started_at)
        {
            unsafe { libc::kill(pid, libc::SIGTERM) };
        }
    }
}

/// Whether a `KERN_PROCARGS2` buffer carries the host marker in its environment,
/// skipping the executable path and arguments that precede it.
pub(crate) fn environment_has_marker(procargs: &[u8]) -> bool {
    let Some((count, rest)) = procargs.split_first_chunk::<4>() else {
        return false;
    };
    let arguments = i32::from_ne_bytes(*count);
    if arguments <= 0 {
        return false;
    }
    let Some(path_end) = rest.iter().position(|byte| *byte == 0) else {
        return false;
    };
    let rest = &rest[path_end + 1..];
    let Some(arguments_start) = rest.iter().position(|byte| *byte != 0) else {
        return false;
    };
    let mut rest = &rest[arguments_start..];
    for _ in 0..arguments {
        let Some(end) = rest.iter().position(|byte| *byte == 0) else {
            return false;
        };
        rest = &rest[end + 1..];
    }
    while let Some(end) = rest.iter().position(|byte| *byte == 0) {
        let entry = &rest[..end];
        if entry.is_empty() {
            break;
        }
        if entry == HOST_MARKER {
            return true;
        }
        rest = &rest[end + 1..];
    }
    false
}

fn user_processes(uid: libc::uid_t) -> Vec<libc::pid_t> {
    let size = unsafe { libc::proc_listpids(PROC_UID_ONLY, uid, std::ptr::null_mut(), 0) };
    let Ok(size) = usize::try_from(size) else {
        return Vec::new();
    };
    let mut pids = vec![0; size / size_of::<libc::pid_t>() + 1];
    loop {
        let Some(bytes) = pids.len().checked_mul(size_of::<libc::pid_t>()) else {
            return Vec::new();
        };
        let Ok(bytes) = i32::try_from(bytes) else {
            return Vec::new();
        };
        let written =
            unsafe { libc::proc_listpids(PROC_UID_ONLY, uid, pids.as_mut_ptr().cast(), bytes) };
        if written <= 0 {
            return Vec::new();
        }
        if written < bytes {
            pids.truncate(written as usize / size_of::<libc::pid_t>());
            return pids;
        }
        pids.resize(pids.len() * 2, 0);
    }
}

fn process_start_time(pid: libc::pid_t, uid: libc::uid_t) -> Option<(u64, u64)> {
    let mut info = MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    let size = size_of::<libc::proc_bsdinfo>() as i32;
    let read = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    if read != size {
        return None;
    }
    let info = unsafe { info.assume_init() };
    (info.pbi_pid == pid as u32 && info.pbi_uid == uid && info.pbi_status != libc::SZOMB)
        .then_some((info.pbi_start_tvsec, info.pbi_start_tvusec))
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

#[cfg(test)]
#[path = "legacy_host_tests.rs"]
mod tests;
