churn_launch() {
    AGENT_DESKTOP_FIXTURE_REF_CHURN=1 AGENT_DESKTOP_FIXTURE_NO_ACTIVATE=1 \
        "$fixture_app/Contents/MacOS/AgentDeskFixture" >/dev/null 2>&1 &
    churn_pid=$!
    fixture_owned=1
}

churn_stop() {
    kill "$churn_pid" 2>/dev/null
    wait "$churn_pid" 2>/dev/null
    fixture_owned=0
}

churn_has_text() {
    [ "$(json_field "$("$bin" wait --text "$1" --app "$app" --timeout 3000 2>/dev/null)" data.found)" = "True" ] && echo 1 || echo 0
}

churn_batch_set_value() {
    local payload
    payload="$(python3 - "$1" "$2" <<'BATCH_PY'
import json, sys
print(json.dumps([{"command": "set-value", "args": {"ref_id": sys.argv[1], "value": sys.argv[2]}}]))
BATCH_PY
)"
    "$bin" batch "$payload" 2>/dev/null
}

churn_refused_as() {
    local output="$1" path="$2" expected="$3" code delivery
    code="$(json_field "$output" "$path.code")"
    delivery="$(json_field "$output" "$path.disposition.delivery")"
    { [ "$code" = "STALE_REF" ] || [ "$code" = "AMBIGUOUS_TARGET" ]; } || return 1
    { [ "$code" = "$expected" ] && [ "$delivery" = "not_delivered" ]; } && echo 1 || echo 0
}

churn_assert_refused() {
    local label="$1" output="$2" path="$3" expected="$4"
    assert "$label" "$(churn_refused_as "$output" "$path" "$expected")" \
        "code=$(json_field "$output" "$path.code") delivery=$(json_field "$output" "$path.disposition.delivery")"
}

churn_expect_fields() {
    local context="$1"
    shift
    local pair
    for pair in "$@"; do
        assert "$context: ${pair%%=*} stored" \
            "$(churn_has_text "${pair%%=*} stored: ${pair#*=}")" \
            "expected '${pair%%=*} stored: ${pair#*=}'"
    done
}

churn_expect_defaults() {
    churn_expect_fields "$1" \
        "Alpha=Alpha" \
        "Beta=Beta" \
        "Stable=Stable value" \
        "Duplicate=Duplicate value" \
        "Gamma=Gamma" \
        "Delta=Delta" \
        "Equal first=Same" \
        "Equal second=Same"
}

churn_live_fields() {
    local native_id="${1:-}" label="${2:-}" output
    if [ -n "$label" ]; then
        output="$("$bin" find --app "$app" --role textfield --limit 50 --native-id "$native_id" --name "$label" 2>/dev/null)"
    elif [ -n "$native_id" ]; then
        output="$("$bin" find --app "$app" --role textfield --limit 50 --native-id "$native_id" 2>/dev/null)"
    else
        output="$("$bin" find --app "$app" --role textfield --limit 50 2>/dev/null)"
    fi
    printf '%s' "$output" | python3 -c '
import json, sys
data = json.loads(sys.stdin.read() or "{}")
matches = data.get("data", {}).get("matches")
if data.get("ok") is not True or not isinstance(matches, list) or not matches:
    raise SystemExit(2)
print(json.dumps(sorted(match.get("value") or "" for match in matches), separators=(",", ":")))
'
}

churn_capture_fields() {
    local variable="$1" values
    values="$(churn_live_fields)" || abort_suite "live text-field values in '$app' could not be read"
    printf -v "$variable" '%s' "$values"
}

churn_field_value() {
    churn_live_fields "${1:-}" "${2:-}" | python3 -c '
import json, sys
values = json.loads(sys.stdin.read() or "[]")
print(values[0] if len(values) == 1 else "")
'
}

churn_expect_live_value() {
    local label="$1" target="$2" native_id="$3" name="$4" expected="$5" output="$6" waited observed
    waited="$("$bin" wait --element "$(target_ref "$target")" --predicate value --value "$expected" --timeout 3000 2>/dev/null)"
    observed="$(churn_field_value "$native_id" "$name")"
    assert "$label" \
        "$([ "$(json_field "$waited" data.found)" = "True" ] && [ "$observed" = "$expected" ] && echo 1 || echo 0)" \
        "expected='$expected' observed='$observed' matched=$(json_field "$waited" data.observed.matched) ok=$(json_field "$output" ok) code=$(json_field "$output" error.code)"
}

churn_assert_no_field_changed() {
    local context="$1" before="$2" after="$3" forbidden="$4" holds
    holds="$(python3 -c '
import json, sys
print(1 if sys.argv[1] in json.loads(sys.argv[2] or "[]") else 0)
' "$forbidden" "$after" 2>/dev/null)"
    assert "$context: every live text field kept its value" \
        "$([ "$before" = "$after" ] && echo 1 || echo 0)" \
        "before=$before after=$after"
    assert "$context: no live text field holds the refused value" \
        "$([ "$holds" = "0" ] && echo 1 || echo 0)" \
        "values=$after refused='$forbidden'"
}

churn_target_nth() {
    local native_id="$1" index="$2" output value
    for _ in $(seq 1 25); do
        output="$("$bin" find --app "$app" --role textfield --native-id "$native_id" --nth "$index" 2>/dev/null)"
        if [ -n "$output" ]; then
            value="$(printf '%s' "$output" | python3 "$json_tool" target 2>/dev/null)"
            if [ -n "$value" ]; then
                printf '%s' "$value"
                return 0
            fi
        fi
        sleep 0.2
    done
    return 1
}

require_target_nth() {
    local variable="$1" native_id="$2" index="$3" value
    value="$(churn_target_nth "$native_id" "$index")" || {
        abort_suite "required fixture target '$native_id' #$index is missing or lacks ref_id + snapshot_id"
    }
    printf -v "$variable" '%s' "$value"
}

note "Default CLI refuses refs it cannot identify"
MODE_FLAG=""

note "Removal: direct and batch refusals leave every field alone"
churn_launch
require_target_by_id filter button filter-rows
require_target_by_id alpha textfield shared-title
act_target "$filter" click >/dev/null 2>&1
assert "filtering applied before the refusal" "$(churn_has_text "Restore rows")" ""
live_count="$("$bin" find --app "$app" --role textfield --native-id shared-title --count 2>/dev/null)"
assert "the filtered layout leaves one shared-title field" \
    "$([ "$(json_field "$live_count" data.count)" = "1" ] && echo 1 || echo 0)" \
    "count=$(json_field "$live_count" data.count)"
churn_capture_fields fields_before
output="$("$bin" set-value "$(target_ref "$alpha")" poisoned 2>/dev/null)"
churn_capture_fields fields_after
churn_assert_refused "removed row is AMBIGUOUS_TARGET without delivery" "$output" error AMBIGUOUS_TARGET
churn_assert_no_field_changed "removal direct" "$fields_before" "$fields_after" "poisoned"
assert "surviving row kept its value after the removal" "$(churn_has_text "Beta stored: Beta")" ""
churn_expect_defaults "removal direct"
churn_capture_fields fields_before
batch_output="$(churn_batch_set_value "$(target_ref "$alpha")" poisoned)"
churn_capture_fields fields_after
churn_assert_refused "batch also refuses the removed row without delivery" \
    "$batch_output" data.results.0.error AMBIGUOUS_TARGET
churn_assert_no_field_changed "removal batch" "$fields_before" "$fields_after" "poisoned"
churn_expect_defaults "removal batch"
churn_stop

note "Replacement: removed row's value reappears on a survivor"
churn_launch
require_target_by_id replace_button button replace-rows
require_target_by_id alpha textfield shared-title
act_target "$replace_button" click >/dev/null 2>&1
assert "replacement applied before the refusal" "$(churn_has_text "Layout: replaced")" ""
churn_capture_fields fields_before
output="$("$bin" set-value "$(target_ref "$alpha")" poisoned 2>/dev/null)"
churn_capture_fields fields_after
churn_assert_refused "replaced row is AMBIGUOUS_TARGET without delivery" "$output" error AMBIGUOUS_TARGET
churn_assert_no_field_changed "replacement direct" "$fields_before" "$fields_after" "poisoned"
assert "replacement kept the value it was given" "$(churn_has_text "Beta stored: Alpha")" ""
churn_expect_fields "replacement direct" \
    "Alpha=Alpha" "Beta=Alpha" "Stable=Stable value" "Duplicate=Duplicate value" \
    "Gamma=Gamma" "Delta=Delta" "Equal first=Same" "Equal second=Same"
churn_capture_fields fields_before
batch_output="$(churn_batch_set_value "$(target_ref "$alpha")" poisoned)"
churn_capture_fields fields_after
churn_assert_refused "batch also refuses the replaced row without delivery" \
    "$batch_output" data.results.0.error AMBIGUOUS_TARGET
churn_assert_no_field_changed "replacement batch" "$fields_before" "$fields_after" "poisoned"
churn_expect_fields "replacement batch" \
    "Alpha=Alpha" "Beta=Alpha" "Stable=Stable value" "Duplicate=Duplicate value" \
    "Gamma=Gamma" "Delta=Delta" "Equal first=Same" "Equal second=Same"
churn_stop

note "Swap: look-alikes trade places"
churn_launch
require_target_by_id swap_button button swap-rows
require_target_by_id alpha textfield shared-title
act_target "$swap_button" click >/dev/null 2>&1
assert "swap applied before the refusal" "$(churn_has_text "Layout: swapped")" ""
churn_capture_fields fields_before
output="$("$bin" set-value "$(target_ref "$alpha")" poisoned 2>/dev/null)"
churn_capture_fields fields_after
churn_assert_refused "swapped row is AMBIGUOUS_TARGET without delivery" "$output" error AMBIGUOUS_TARGET
churn_assert_no_field_changed "swap direct" "$fields_before" "$fields_after" "poisoned"
churn_expect_defaults "swap direct"
churn_capture_fields fields_before
batch_output="$(churn_batch_set_value "$(target_ref "$alpha")" poisoned)"
churn_capture_fields fields_after
churn_assert_refused "batch also refuses the swapped row without delivery" \
    "$batch_output" data.results.0.error AMBIGUOUS_TARGET
churn_assert_no_field_changed "swap batch" "$fields_before" "$fields_after" "poisoned"
churn_expect_defaults "swap batch"
churn_stop

note "Unique editor gains a live duplicate"
churn_launch
require_target_by_id stable textfield stable-editor
require_target_by_id duplicate_button button add-duplicate
first_write="$("$bin" set-value "$(target_ref "$stable")" stable-ok 2>/dev/null)"
churn_expect_live_value "unique editor accepts its first write" \
    "$stable" stable-editor "" stable-ok "$first_write"
second_write="$("$bin" set-value "$(target_ref "$stable")" stable-ok-again 2>/dev/null)"
churn_expect_live_value "unique editor accepts a second write through the same ref" \
    "$stable" stable-editor "" stable-ok-again "$second_write"
churn_expect_defaults "unique editor writes"
act_target "$duplicate_button" click >/dev/null 2>&1
assert "duplicate applied before the refusal" "$(churn_has_text "Layout: duplicated")" ""
churn_capture_fields fields_before
output="$("$bin" set-value "$(target_ref "$stable")" poisoned 2>/dev/null)"
churn_capture_fields fields_after
churn_assert_refused "ref with a live duplicate is AMBIGUOUS_TARGET without delivery" "$output" error AMBIGUOUS_TARGET
churn_assert_no_field_changed "duplicate direct" "$fields_before" "$fields_after" "poisoned"
churn_expect_defaults "duplicate direct"
churn_capture_fields fields_before
batch_output="$(churn_batch_set_value "$(target_ref "$stable")" poisoned)"
churn_capture_fields fields_after
churn_assert_refused "batch also reports AMBIGUOUS_TARGET without delivery" \
    "$batch_output" data.results.0.error AMBIGUOUS_TARGET
churn_assert_no_field_changed "duplicate batch" "$fields_before" "$fields_after" "poisoned"
churn_expect_defaults "duplicate batch"
churn_stop

note "Equal values under one identifier: every capture is refused after one field disappears"
churn_launch
require_target_nth equal_first equal-title 0
require_target_nth equal_second equal-title 1
pair_count="$("$bin" find --app "$app" --role textfield --native-id equal-title --count 2>/dev/null)"
assert "the equal-valued pair shares one identifier across two live fields" \
    "$([ "$(json_field "$pair_count" data.count)" = "2" ] && echo 1 || echo 0)" \
    "count=$(json_field "$pair_count" data.count)"
require_target_by_id equal_remove button remove-equal
act_target "$equal_remove" click >/dev/null 2>&1
assert "equal removal applied before the refusal" "$(churn_has_text "Equal layout: removed")" ""
live_count="$("$bin" find --app "$app" --role textfield --native-id equal-title --count 2>/dev/null)"
assert "one equal-valued field survives the removal" \
    "$([ "$(json_field "$live_count" data.count)" = "1" ] && echo 1 || echo 0)" \
    "count=$(json_field "$live_count" data.count)"
churn_capture_fields fields_before
output="$("$bin" set-value "$(target_ref "$equal_first")" poisoned 2>/dev/null)"
churn_capture_fields fields_after
churn_assert_refused "equal-valued first capture is AMBIGUOUS_TARGET without delivery" \
    "$output" error AMBIGUOUS_TARGET
churn_assert_no_field_changed "equal direct" "$fields_before" "$fields_after" "poisoned"
churn_expect_defaults "equal direct"
churn_capture_fields fields_before
batch_output="$(churn_batch_set_value "$(target_ref "$equal_second")" poisoned)"
churn_capture_fields fields_after
churn_assert_refused "batch also refuses the equal-valued second capture" \
    "$batch_output" data.results.0.error AMBIGUOUS_TARGET
churn_assert_no_field_changed "equal batch" "$fields_before" "$fields_after" "poisoned"
churn_expect_defaults "equal batch"
churn_stop

note "Description-distinguished pair: not expressible from SwiftUI on macOS"
skipmsg "description-distinguished sub-case skipped: agent-desktop reads description only from kAXDescriptionAttribute (crates/macos/src/tree/properties/node_attribute_fetch.rs), .accessibilityHint and .help set AXHelp only (measured on this fixture with AXUIElementCopyAttributeValue), and .accessibilityLabel writes AXDescription, a name source that outranks AXPlaceholderValue, so no SwiftUI modifier yields a distinct description without also setting the name"

note "Labeled look-alikes: a name-based ref addresses its own field"
churn_launch
require_target_by_id gamma textfield labeled-title "Gamma label"
first_write="$(act_target "$gamma" set-value gamma-ok 2>/dev/null)"
churn_expect_live_value "labeled field accepts a write through its name-based ref" \
    "$gamma" labeled-title "Gamma label" gamma-ok "$first_write"
second_write="$("$bin" set-value "$(target_ref "$gamma")" gamma-ok-2 2>/dev/null)"
churn_expect_live_value "labeled field accepts a second write through the same ref" \
    "$gamma" labeled-title "Gamma label" gamma-ok-2 "$second_write"
churn_expect_defaults "labeled writes"
churn_stop

note "Labeled removal: the removed field is STALE_REF while its look-alike takes its place"
churn_launch
require_target_by_id gamma textfield labeled-title "Gamma label"
require_target_by_id labeled_filter button filter-labeled
act_target "$labeled_filter" click >/dev/null 2>&1
assert "labeled removal applied before the refusal" "$(churn_has_text "Layout: labeled-filtered")" ""
live_count="$("$bin" find --app "$app" --role textfield --native-id labeled-title --count 2>/dev/null)"
assert "the surviving look-alike is the single live match for the shared identifier" \
    "$([ "$(json_field "$live_count" data.count)" = "1" ] && echo 1 || echo 0)" \
    "count=$(json_field "$live_count" data.count)"
churn_capture_fields fields_before
output="$("$bin" set-value "$(target_ref "$gamma")" poisoned 2>/dev/null)"
churn_capture_fields fields_after
churn_assert_refused "removed labeled field is STALE_REF without delivery" "$output" error STALE_REF
churn_assert_no_field_changed "labeled removal direct" "$fields_before" "$fields_after" "poisoned"
churn_expect_defaults "labeled removal direct"
churn_capture_fields fields_before
batch_output="$(churn_batch_set_value "$(target_ref "$gamma")" poisoned)"
churn_capture_fields fields_after
churn_assert_refused "batch also reports STALE_REF without delivery" \
    "$batch_output" data.results.0.error STALE_REF
churn_assert_no_field_changed "labeled removal batch" "$fields_before" "$fields_after" "poisoned"
churn_expect_defaults "labeled removal batch"
churn_stop

note "Unlabeled look-alikes are refused before any change"
churn_launch
require_target_by_id alpha textfield shared-title
churn_capture_fields fields_before
output="$("$bin" set-value "$(target_ref "$alpha")" poisoned 2>/dev/null)"
churn_capture_fields fields_after
churn_assert_refused "unlabeled shared-title ref is AMBIGUOUS_TARGET without delivery" \
    "$output" error AMBIGUOUS_TARGET
churn_assert_no_field_changed "unlabeled direct" "$fields_before" "$fields_after" "poisoned"
churn_expect_defaults "unlabeled direct"
churn_capture_fields fields_before
batch_output="$(churn_batch_set_value "$(target_ref "$alpha")" poisoned)"
churn_capture_fields fields_after
churn_assert_refused "batch also refuses the unlabeled shared-title ref" \
    "$batch_output" data.results.0.error AMBIGUOUS_TARGET
churn_assert_no_field_changed "unlabeled batch" "$fields_before" "$fields_after" "poisoned"
churn_expect_defaults "unlabeled batch"
churn_stop

note "Complete app inventory gives a terminal unknown-app response"
run_timed "$bin" snapshot --app "AgentDeskMissing-$churn_pid" --timeout-ms 1500
assert "unknown app is APP_NOT_FOUND without delivery" \
    "$([ "$(json_field "$RUN_JSON" error.code)" = "APP_NOT_FOUND" ] && [ "$(json_field "$RUN_JSON" error.disposition.delivery)" = "not_delivered" ] && echo 1 || echo 0)" \
    "exit=$RUN_EXIT elapsed=$RUN_MS code=$(json_field "$RUN_JSON" error.code)"
