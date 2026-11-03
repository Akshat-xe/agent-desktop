#!/bin/sh
set -eu

here=$(cd "$(dirname "$0")" && pwd)
tmp=$(mktemp -d "${TMPDIR:-/tmp}/ad macos vm provider.XXXXXX")
cleanup() {
  for pid in "${source_pid:-}" "${foreign_pid:-}" "${vm_pid:-}" "${job_pid:-}"; do
    [ -n "$pid" ] || continue
    kill "$pid" 2>/dev/null || true
  done
  chmod -R u+rwX "$tmp" 2>/dev/null || true
  rm -rf "$tmp"
}
trap cleanup 0
trap 'exit 1' 1 2 3 15
mkdir -p "$tmp/bin" "$tmp/provider/build" "$tmp/sandboxes/demo"

AD_SANDBOX_NAME=demo
AD_SANDBOX_DIR=$tmp/sandboxes/demo
AD_PROVIDER_DIR=$tmp/provider
AD_BINARY=$tmp/agent-desktop
VMHOST=$tmp/bin/vmhost
FAKE_LOG=$tmp/calls
FAKE_UNAME_M=arm64
FAKE_SWIFTC_FOUND=1
FAKE_MACOS_VERSION=14.0
FAKE_HV_SUPPORT=1
FAKE_SWIFTC_OK=0
FAKE_GUEST_SHA1=2222222222222222222222222222222222222222
FAKE_SSH_PERMISSIONS='{"ok":true,"data":{"accessibility":{"state":"granted"}}}'
FAKE_SSH_EXEC_OUTPUT='{"ok":true,"command":"guest"}'
FAKE_PS_COMM=/opt/agent-desktop/build/vmhost
FAKE_PS_BUNDLE=$AD_SANDBOX_DIR/vm
export AD_SANDBOX_NAME AD_SANDBOX_DIR AD_PROVIDER_DIR AD_BINARY VMHOST
export FAKE_LOG FAKE_UNAME_M FAKE_SWIFTC_FOUND
export FAKE_MACOS_VERSION FAKE_HV_SUPPORT FAKE_SWIFTC_OK
export FAKE_GUEST_SHA1 FAKE_PS_BUNDLE FAKE_PS_COMMAND
export FAKE_SSH_PERMISSIONS
export FAKE_SSH_EXEC_OUTPUT FAKE_PS_COMM

printf '#!/bin/sh\n' > "$AD_PROVIDER_DIR/guest-setup.sh"
printf 'fake source\n' > "$AD_PROVIDER_DIR/vmhost.swift"
printf 'fake entitlements\n' > "$AD_PROVIDER_DIR/vm.entitlements"

printf 'binary\n' > "$AD_BINARY"

cat > "$tmp/bin/uname" <<'FAKE'
#!/bin/sh
printf '%s\n' "${FAKE_UNAME_M:-arm64}"
FAKE
cat > "$tmp/bin/sw_vers" <<'FAKE'
#!/bin/sh
printf '%s\n' "${FAKE_MACOS_VERSION:-14.0}"
FAKE
cat > "$tmp/bin/sysctl" <<'FAKE'
#!/bin/sh
printf '%s\n' "${FAKE_HV_SUPPORT:-1}"
FAKE
cat > "$tmp/bin/xcrun" <<'FAKE'
#!/bin/sh
if [ "${1:-}" = --find ]; then
  [ "${FAKE_SWIFTC_FOUND:-1}" = 1 ] || exit 1
  printf '%s\n' "${FAKE_SWIFTC_PATH:-$FAKE_BIN_DIR/swiftc}"
  exit 0
fi
if [ "${1:-}" = swiftc ]; then
  shift
  exec "$FAKE_BIN_DIR/swiftc" "$@"
fi
exit 2
FAKE
cat > "$tmp/bin/swiftc" <<'FAKE'
#!/bin/sh
set -eu
printf 'swiftc\n' >> "$FAKE_LOG"
[ "${FAKE_SWIFTC_OK:-0}" = 1 ] || exit 1
output=
while [ "$#" -gt 0 ]; do
  if [ "$1" = -o ]; then shift; output=$1; fi
  shift
done
printf '#!/bin/sh\nexit 0\n' > "$output"
chmod 0755 "$output"
FAKE
cat > "$tmp/bin/codesign" <<'FAKE'
#!/bin/sh
printf 'com.apple.security.virtualization\n'
FAKE
cat > "$tmp/bin/vmhost" <<'FAKE'
#!/bin/sh
set -eu
{
  printf 'BEGIN vmhost\n'
  for arg in "$@"; do printf '%s\n' "$arg"; done
  printf 'END\n'
} >> "$FAKE_LOG"
case ${1:-} in
  install) exit 1 ;;
  clone)
    mkdir -p "$3/share"
    : > "$3/Disk.img"
    : > "$3/AuxiliaryStorage"
    : > "$3/HardwareModel"
    : > "$3/MachineIdentifier"
    : > "$3/config.json"
    ;;
  ip) printf '{"ok":true,"ip":"192.0.2.2"}\n' ;;
  *) exit 2 ;;
esac
FAKE
cat > "$tmp/bin/ssh-keygen" <<'FAKE'
#!/bin/sh
set -eu
while [ "$#" -gt 0 ]; do
  if [ "$1" = -f ]; then shift; key=$1; fi
  shift
done
printf 'private key\n' > "$key"
printf 'public key\n' > "$key.pub"
FAKE
cat > "$tmp/bin/curl" <<'FAKE'
#!/bin/sh
set -eu
{
  printf 'BEGIN curl\n'
  for arg in "$@"; do printf '%s\n' "$arg"; done
  printf 'END\n'
} >> "$FAKE_LOG"
exit 1
FAKE
cat > "$tmp/bin/ssh" <<'FAKE'
#!/bin/sh
set -eu
{
  printf 'BEGIN ssh\n'
  for arg in "$@"; do printf '%s\n' "$arg"; done
  printf 'END\n'
} >> "$FAKE_LOG"
remote=
for arg in "$@"; do remote=$arg; done
case $remote in
  *'shasum -a 1'*) printf '%s  agent-desktop\n' "${FAKE_GUEST_SHA1:-}" ;;
  *'agent-desktop permissions'*)
    printf '%s\n' "$FAKE_SSH_PERMISSIONS"
    ;;
  *)
    printf '%s\n' "${FAKE_SSH_EXEC_OUTPUT:-}"
    ;;
esac
FAKE
cat > "$tmp/bin/scp" <<'FAKE'
#!/bin/sh
{
  printf 'BEGIN scp\n'
  for arg in "$@"; do printf '%s\n' "$arg"; done
  printf 'END\n'
} >> "$FAKE_LOG"
FAKE
cat > "$tmp/bin/ps" <<'FAKE'
#!/bin/sh
case " $* " in
  *" command="*) printf '%s\n' "${FAKE_PS_COMMAND:-/opt/agent-desktop/build/vmhost run $FAKE_PS_BUNDLE}" ;;
  *) printf '%s\n' "${FAKE_PS_COMM:-/opt/agent-desktop/build/vmhost}" ;;
esac
FAKE
chmod 0755 "$tmp/bin/"*
FAKE_BIN_DIR=$tmp/bin
PATH=$tmp/bin:$PATH
export FAKE_BIN_DIR PATH

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

run_provider() {
  sh "$here/provider.sh" "$@"
}

run_provider_with() {
  setting=$1
  value=$2
  shift 2
  env "$setting=$value" sh "$here/provider.sh" "$@"
}

wait_for_exit() {
  wait_tries=0
  while kill -0 "$1" 2>/dev/null && [ "$wait_tries" -lt 100 ]; do
    sleep 0.1
    wait_tries=$((wait_tries + 1))
  done
}

assert_field() {
  actual=$(python3 -c 'import json,sys; print(json.loads(open(sys.argv[1]).readlines()[-1])[sys.argv[2]])' "$1" "$2")
  [ "$actual" = "$3" ] || fail "$4: expected $2=[$3] but saw [$actual]"
}

run_ok() {
  status=0
  run_provider "$@" > "$tmp/out" 2> "$tmp/err" || status=$?
  [ "$status" = 0 ] || fail "provider $1 exited $status: $(cat "$tmp/out" "$tmp/err")"
}

run_ok_with() {
  setting=$1
  value=$2
  verb=$3
  shift 3
  status=0
  run_provider_with "$setting" "$value" "$verb" "$@" > "$tmp/out" 2> "$tmp/err" || status=$?
  [ "$status" = 0 ] || fail "provider $verb exited $status: $(cat "$tmp/out" "$tmp/err")"
}

FAKE_UNAME_M=x86_64
run_ok check
assert_field "$tmp/out" state unavailable 'Intel host check'

FAKE_UNAME_M=arm64
FAKE_HV_SUPPORT=0
run_ok check
assert_field "$tmp/out" state unavailable 'hypervisor check'
assert_field "$tmp/out" message 'This Mac cannot run virtual machines (it may itself be a virtual machine).' 'hypervisor check message'
! grep -q 'install macos-vm' "$tmp/out" || fail 'hypervisor check suggested an install'
FAKE_HV_SUPPORT=1

FAKE_SWIFTC_FOUND=0
run_ok check
assert_field "$tmp/out" state unavailable 'missing Command Line Tools check'

FAKE_SWIFTC_FOUND=1
printf '#!/bin/sh\nexit 0\n' > "$AD_PROVIDER_DIR/build/vmhost"
chmod 0755 "$AD_PROVIDER_DIR/build/vmhost"
: > "$FAKE_LOG"
for verb in check install; do
  run_ok "$verb"
  assert_field "$tmp/out" state ready 'built and signed host'
done
! grep -q '^swiftc$' "$FAKE_LOG" || fail 'ready host compiled vmhost'

rm -f "$AD_PROVIDER_DIR/build/vmhost"
FAKE_SWIFTC_OK=1
run_ok install
assert_field "$tmp/out" message 'The macOS VM host was built and signed; the provider is ready.' 'install after building'
FAKE_SWIFTC_OK=0

status=0
run_provider_with AD_SET_FROM ghost create demo > "$tmp/out" 2> "$tmp/err" || status=$?
[ "$status" = 1 ] || fail "create from a missing sandbox exited $status"
assert_field "$tmp/out" code SANDBOX_NOT_FOUND 'create from a missing sandbox'
assert_field "$tmp/out" message 'No sandbox named ghost exists.' 'create from a missing sandbox message'
assert_field "$tmp/out" suggestion 'Run `agent-desktop sandbox list` to see the sandboxes that exist.' 'create from a missing sandbox suggestion'

source_dir=$tmp/sandboxes/source
mkdir -p "$source_dir/vm/share"
: > "$source_dir/vm/Disk.img"
: > "$source_dir/vm/AuxiliaryStorage"
: > "$source_dir/vm/HardwareModel"
: > "$source_dir/vm/MachineIdentifier"
: > "$source_dir/vm/config.json"
printf '#!/bin/sh\n' > "$source_dir/vm/share/guest-setup.sh"
printf 'source public key\n' > "$source_dir/vm/share/id_ed25519.pub"
printf 'binary\n' > "$source_dir/vm/share/agent-desktop"
printf 'source setup marker\n' > "$source_dir/setup_done"
printf 'source key\n' > "$source_dir/id_ed25519"
printf 'source public key\n' > "$source_dir/id_ed25519.pub"
printf 'source hosts\n' > "$source_dir/known_hosts"
printf 'guest\n' > "$source_dir/guest_user"
: > "$FAKE_LOG"
run_ok_with AD_SET_FROM source create demo
python3 - "$tmp/out" "$FAKE_LOG" "$source_dir" "$AD_SANDBOX_DIR" <<'PY'
import json
import pathlib
import sys

reply, log, source, target = map(pathlib.Path, sys.argv[1:])
assert (
    json.loads(reply.read_text().splitlines()[-1])["state"] == "stopped"
    and "BEGIN curl" not in log.read_text().splitlines()
    and "clone" in log.read_text().splitlines()
    and all((target / name).read_bytes() == (source / name).read_bytes()
            for name in ("id_ed25519", "known_hosts", "setup_done"))
), "create did not clone a set-up stopped source without downloading"
PY

printf '999999999\n' > "$source_dir/vm/vmhost.pid"
rm -rf "$AD_SANDBOX_DIR"
mkdir -p "$AD_SANDBOX_DIR"
run_ok_with AD_SET_FROM source create demo
assert_field "$tmp/out" state stopped 'create from a source whose VM died'
[ ! -f "$source_dir/vm/vmhost.pid" ] || fail 'create kept the dead record of its source'

sleep 30 &
source_pid=$!
printf '%s\n' "$source_pid" > "$source_dir/vm/vmhost.pid"
FAKE_PS_COMM=/opt/agent-desktop/build/vmhost
FAKE_PS_COMMAND="/opt/agent-desktop/build/vmhost run $source_dir/vm"
rm -rf "$AD_SANDBOX_DIR"
mkdir -p "$AD_SANDBOX_DIR"
status=0
run_provider_with AD_SET_FROM source create demo > "$tmp/out" 2> "$tmp/err" || status=$?
[ "$status" = 1 ] && kill -0 "$source_pid" 2>/dev/null || fail 'create did not refuse a running source safely'
kill "$source_pid" 2>/dev/null || true
wait "$source_pid" 2>/dev/null || true
source_pid=
FAKE_PS_COMMAND=

user_ipsw=$tmp/user.ipsw
printf 'user image\n' > "$user_ipsw"
rm -rf "$AD_SANDBOX_DIR"
mkdir -p "$AD_SANDBOX_DIR"
run_ok_with AD_SET_IPSW "$user_ipsw" create demo
job_pid=$(cat "$AD_SANDBOX_DIR/vm/install.pid")
wait_for_exit "$job_pid"
job_pid=
run_ok status demo
[ -f "$user_ipsw" ] || fail 'failed installation deleted the user-supplied image'

mkdir -p "$AD_SANDBOX_DIR/vm"
sleep 30 &
foreign_pid=$!
printf '%s\n' "$foreign_pid" > "$AD_SANDBOX_DIR/vm/vmhost.pid"
FAKE_PS_COMM=/usr/bin/sleep
run_ok stop demo
assert_field "$tmp/out" state broken 'stop of a foreign pid'
kill -0 "$foreign_pid" 2>/dev/null || fail 'stop signalled a pid whose command was not vmhost'
[ -f "$AD_SANDBOX_DIR/vm/vmhost.pid" ] || fail 'stop discarded a live foreign VM process record'
kill "$foreign_pid" 2>/dev/null || true
wait "$foreign_pid" 2>/dev/null || true
foreign_pid=

mkdir -p "$AD_SANDBOX_DIR/vm"
sleep 30 &
foreign_pid=$!
printf '%s\n' "$foreign_pid" > "$AD_SANDBOX_DIR/vm/vmhost.pid"
FAKE_PS_COMM=/opt/agent-desktop/build/vmhost
FAKE_PS_COMMAND="/opt/agent-desktop/build/vmhost run $tmp/sandboxes/other/vm"
run_ok stop demo
assert_field "$tmp/out" state broken 'stop of a vmhost pid that runs another bundle'
assert_field "$tmp/out" message 'Sandbox demo has a dead or foreign VM process record that was not signalled.' 'stop of a vmhost pid that runs another bundle message'
kill -0 "$foreign_pid" 2>/dev/null || fail 'stop signalled a vmhost pid that runs another bundle'
[ -f "$AD_SANDBOX_DIR/vm/vmhost.pid" ] || fail 'stop discarded a foreign VM process record for another bundle'
kill "$foreign_pid" 2>/dev/null || true
wait "$foreign_pid" 2>/dev/null || true
foreign_pid=
FAKE_PS_COMMAND=

FAKE_PS_COMM=/opt/agent-desktop/build/vmhost
mkdir -p "$AD_SANDBOX_DIR/vm/share"
: > "$AD_SANDBOX_DIR/vm/Disk.img"
: > "$AD_SANDBOX_DIR/vm/AuxiliaryStorage"
: > "$AD_SANDBOX_DIR/vm/HardwareModel"
: > "$AD_SANDBOX_DIR/vm/MachineIdentifier"
: > "$AD_SANDBOX_DIR/vm/config.json"
printf '#!/bin/sh\n' > "$AD_SANDBOX_DIR/vm/share/guest-setup.sh"
printf 'public key\n' > "$AD_SANDBOX_DIR/vm/share/id_ed25519.pub"
cp "$AD_BINARY" "$AD_SANDBOX_DIR/vm/share/agent-desktop"
rm -f "$AD_SANDBOX_DIR/vm/install.pid" "$AD_SANDBOX_DIR/vm/install.done"
printf '999999999\n' > "$AD_SANDBOX_DIR/vm/vmhost.pid"
run_ok status demo
assert_field "$tmp/out" state stopped 'status dead pid'
assert_field "$tmp/out" message 'Sandbox demo is stopped; its VM did not shut down cleanly.' 'status dead pid message'
assert_field "$tmp/out" next 'agent-desktop sandbox start demo' 'status dead pid next'
[ ! -f "$AD_SANDBOX_DIR/vm/vmhost.pid" ] || fail 'status kept the dead VM process record'

printf '999999999\n' > "$AD_SANDBOX_DIR/vm/vmhost.pid"
status=0
run_provider start demo > "$tmp/out" 2> "$tmp/err" || status=$?
[ ! -f "$AD_SANDBOX_DIR/vm/vmhost.pid" ] || fail 'start kept the dead VM process record'
! grep -q 'dead or foreign' "$tmp/out" || fail 'start refused to launch over a dead VM process record'

printf '1000000000\n' > "$AD_SANDBOX_DIR/vm/download.size"
mkfile -n 340000000 "$AD_SANDBOX_DIR/vm/latest.ipsw"
printf '%s\n' "$AD_SANDBOX_DIR/vm/latest.ipsw" > "$AD_SANDBOX_DIR/vm/download.path"
sleep 30 &
job_pid=$!
printf '%s\n' "$job_pid" > "$AD_SANDBOX_DIR/vm/install.pid"
run_ok_with FAKE_PS_COMMAND "$here/provider.sh __install_job download" status demo
grep -q 'is downloading the macOS image: 0.3 of 1.0 GB (34%)\.' "$tmp/out" || \
  fail "status did not report download progress: $(cat "$tmp/out")"
kill "$job_pid" 2>/dev/null || true
wait "$job_pid" 2>/dev/null || true
job_pid=
rm -f "$AD_SANDBOX_DIR/vm/install.pid" "$AD_SANDBOX_DIR/vm/download.size" "$AD_SANDBOX_DIR/vm/download.path"

sleep 30 &
job_pid=$!
printf '%s\n' "$job_pid" > "$AD_SANDBOX_DIR/vm/install.pid"
status=0
run_provider_with FAKE_PS_COMMAND "$here/provider.sh __install_job download" rm demo > "$tmp/out" 2> "$tmp/err" || status=$?
[ "$status" = 1 ] || fail "rm while installing exited $status"
assert_field "$tmp/out" code ACTION_FAILED 'rm while installing'
assert_field "$tmp/out" message 'Sandbox demo is still installing, so it was not removed.' 'rm while installing message'
assert_field "$tmp/out" suggestion 'Run `agent-desktop sandbox status demo` and remove it after the install finishes or fails.' 'rm while installing suggestion'
kill -0 "$job_pid" 2>/dev/null || fail 'rm killed the install job'
[ -d "$AD_SANDBOX_DIR/vm" ] || fail 'rm removed the bundle while the install job was running'
kill "$job_pid" 2>/dev/null || true
wait "$job_pid" 2>/dev/null || true
job_pid=
rm -f "$AD_SANDBOX_DIR/vm/install.pid"

sleep 30 &
vm_pid=$!
printf '%s\n' "$vm_pid" > "$AD_SANDBOX_DIR/vm/vmhost.pid"
: > "$FAKE_LOG"
run_ok status demo
[ "$(grep -c '^BEGIN ssh$' "$FAKE_LOG" || true)" = 1 ] || fail 'status did not use exactly one SSH connection'

weird='semi; double "quoted" and '\''single'\'' $(touch /tmp/should-not-run)'
: > "$FAKE_LOG"
run_provider exec demo -- "$weird" > "$tmp/out" 2> "$tmp/err"
python3 - "$FAKE_LOG" "$weird" <<'PY'
import shlex
import sys

blocks = []
current = None
for line in open(sys.argv[1]):
    line = line.rstrip("\n")
    if line == "BEGIN ssh":
        current = []
    elif line == "END":
        if current is not None:
            blocks.append(current)
        current = None
    elif current is not None:
        current.append(line)
if not blocks:
    sys.exit("no ssh command was recorded")
if shlex.split(blocks[-1][-1]) != ["~/.local/bin/agent-desktop", sys.argv[2]]:
    sys.exit("exec argument was not preserved as one quoted word: %r" % blocks[-1][-1])
PY
kill "$vm_pid" 2>/dev/null || true
wait "$vm_pid" 2>/dev/null || true
vm_pid=
printf '999999999\n' > "$AD_SANDBOX_DIR/vm/vmhost.pid"
status=0
run_provider exec demo -- snapshot > "$tmp/out" 2> "$tmp/err" || status=$?
[ "$status" = 125 ] || fail "exec on a dead sandbox exited $status"
grep -q 'Sandbox demo is stopped\.' "$tmp/out" || fail 'exec on a dead VM process record did not report a stopped sandbox'
[ ! -f "$AD_SANDBOX_DIR/vm/vmhost.pid" ] || fail 'exec kept the dead VM process record'

rm -f "$AD_SANDBOX_DIR/vm/vmhost.pid"
status=0
run_provider exec demo -- snapshot > "$tmp/out" 2> "$tmp/err" || status=$?
[ "$status" = 125 ] || fail "exec on a stopped sandbox exited $status"

printf 'provider tests passed\n'
