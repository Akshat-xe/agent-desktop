//! Runs one tool call as a child process of this same binary and maps its JSON envelope to an MCP result.

use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, MutexGuard, PoisonError};

use super::catalog::Tool;

const CANCELLED: &str = "Cancelled: the command was killed before it finished";

/// The calls in flight, keyed by JSON-RPC request id, with their child once it has started.
pub(crate) type Running = Mutex<HashMap<String, Option<Child>>>;

/// Registers request `id` before its call starts, so a cancel that arrives first still stops it.
pub(crate) fn admit(running: &Running, id: &Value) {
    lock(running).insert(id.to_string(), None);
}

/// Runs `tool` with `arguments`, prefixed by the server's own global flags, and returns a `CallToolResult`.
/// A call whose `id` was not admitted, or was cancelled, is killed and answers `Cancelled`.
pub(crate) fn call(
    tool: &Tool,
    arguments: &Map<String, Value>,
    globals: &[String],
    running: &Running,
    id: &Value,
) -> Value {
    let key = id.to_string();
    let result = run(tool, arguments, globals, running, &key);
    lock(running).remove(&key);
    result
}

fn run(
    tool: &Tool,
    arguments: &Map<String, Value>,
    globals: &[String],
    running: &Running,
    key: &str,
) -> Value {
    let argv = match tool.argv(arguments) {
        Ok(argv) => argv,
        Err(message) => return error_result(&message),
    };
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(error) => return error_result(&format!("Cannot locate agent-desktop: {error}")),
    };
    let mut child = match Command::new(exe)
        .args(globals)
        .args(&argv)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => return error_result(&format!("Cannot run agent-desktop: {error}")),
    };
    let pipe = child.stdout.take();
    if let Some(slot) = lock(running).get_mut(key) {
        *slot = Some(child);
    } else {
        kill(child);
        return error_result(CANCELLED);
    }
    let mut stdout = Vec::new();
    if let Some(mut pipe) = pipe {
        let _ = pipe.read_to_end(&mut stdout);
    }
    let Some(mut child) = lock(running).get_mut(key).and_then(Option::take) else {
        return error_result(CANCELLED);
    };
    let status = child.wait();
    match serde_json::from_slice::<Value>(&stdout) {
        Ok(envelope) => envelope_result(envelope),
        Err(_) => error_result(&format!(
            "agent-desktop {} exited with {} and no JSON envelope; its stderr is in the server's stderr",
            tool.path.join(" "),
            status.map_or_else(|error| error.to_string(), |status| status.to_string())
        )),
    }
}

/// Cancels request `id`, killing its child if it has started.
pub(crate) fn cancel(running: &Running, id: &Value) {
    let child = lock(running).remove(&id.to_string()).flatten();
    child.into_iter().for_each(kill);
}

fn kill(mut child: Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn lock(running: &Running) -> MutexGuard<'_, HashMap<String, Option<Child>>> {
    running.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Maps an envelope to a result: the envelope as structured content and text, an image block for inline screenshots.
pub(crate) fn envelope_result(mut envelope: Value) -> Value {
    let is_error = envelope["ok"] != Value::Bool(true);
    let image = take_inline_image(&mut envelope);
    let mut content = vec![json!({ "type": "text", "text": envelope.to_string() })];
    content.extend(image);
    json!({ "content": content, "structuredContent": envelope, "isError": is_error })
}

fn take_inline_image(envelope: &mut Value) -> Option<Value> {
    let data = envelope.get_mut("data")?.as_object_mut()?;
    let mime_type = match data.get("format")?.as_str()? {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        _ => return None,
    };
    if !data.get("data").is_some_and(Value::is_string) {
        return None;
    }
    let Value::String(encoded) = data.remove("data")? else {
        return None;
    };
    Some(json!({ "type": "image", "data": encoded, "mimeType": mime_type }))
}

fn error_result(message: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": message }], "isError": true })
}
