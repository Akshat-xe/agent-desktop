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

churn_refused() {
    local code delivery
    code="$(json_field "$1" "$2.code")"
    delivery="$(json_field "$1" "$2.disposition.delivery")"
    { { [ "$code" = "STALE_REF" ] || [ "$code" = "AMBIGUOUS_TARGET" ]; } && [ "$delivery" = "not_delivered" ]; } && echo 1 || echo 0
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

note "Default CLI refuses refs it cannot identify"
MODE_FLAG=""
churn_launch
require_target_by_id filter button filter-rows
require_target_by_id alpha textfield shared-title
act_target "$filter" click >/dev/null 2>&1
"$bin" wait --text "Restore rows" --app "$app" --timeout 5000 >/dev/null 2>&1
output="$("$bin" set-value "$(target_ref "$alpha")" poisoned 2>/dev/null)"
assert "removed row is refused without delivery" "$(churn_refused "$output" error)" \
    "code=$(json_field "$output" error.code) delivery=$(json_field "$output" error.disposition.delivery)"
batch_output="$(churn_batch_set_value "$(target_ref "$alpha")" poisoned)"
assert "batch also refuses the removed row without delivery" \
    "$(churn_refused "$batch_output" data.results.0.error)" \
    "code=$(json_field "$batch_output" data.results.0.error.code)"
assert "surviving row kept its value after the removal" "$(churn_has_text "Beta stored: Beta")" ""
assert "removed row's value is untouched" "$(churn_has_text "Alpha stored: Alpha")" ""
churn_stop

note "Replacement: removed row's value reappears on a survivor"
churn_launch
require_target_by_id replace_button button replace-rows
require_target_by_id alpha textfield shared-title
act_target "$replace_button" click >/dev/null 2>&1
"$bin" wait --text "Layout: replaced" --app "$app" --timeout 5000 >/dev/null 2>&1
output="$("$bin" set-value "$(target_ref "$alpha")" poisoned 2>/dev/null)"
assert "replaced row is refused without delivery" "$(churn_refused "$output" error)" \
    "code=$(json_field "$output" error.code) delivery=$(json_field "$output" error.disposition.delivery)"
assert "replacement kept the value it was given" "$(churn_has_text "Beta stored: Alpha")" ""
assert "removed row's value is untouched after replacement" "$(churn_has_text "Alpha stored: Alpha")" ""
churn_stop

note "Swap: look-alikes trade places"
churn_launch
require_target_by_id swap_button button swap-rows
require_target_by_id alpha textfield shared-title
act_target "$swap_button" click >/dev/null 2>&1
"$bin" wait --text "Layout: swapped" --app "$app" --timeout 5000 >/dev/null 2>&1
output="$("$bin" set-value "$(target_ref "$alpha")" poisoned 2>/dev/null)"
assert "swapped row is refused without delivery" "$(churn_refused "$output" error)" \
    "code=$(json_field "$output" error.code) delivery=$(json_field "$output" error.disposition.delivery)"
assert "first row kept its value after the swap" "$(churn_has_text "Alpha stored: Alpha")" ""
assert "second row kept its value after the swap" "$(churn_has_text "Beta stored: Beta")" ""
churn_stop

note "Unique editor gains a live duplicate"
churn_launch
require_target_by_id stable textfield stable-editor
require_target_by_id duplicate_button button add-duplicate
first_write="$("$bin" set-value "$(target_ref "$stable")" stable-ok 2>/dev/null)"
assert "unique editor accepts its first write" \
    "$([ "$(json_field "$first_write" ok)" = "True" ] && [ "$(churn_has_text "Stable stored: stable-ok")" = "1" ] && echo 1 || echo 0)" \
    "ok=$(json_field "$first_write" ok)"
second_write="$("$bin" set-value "$(target_ref "$stable")" stable-ok-again 2>/dev/null)"
assert "unique editor accepts a second write through the same ref" \
    "$([ "$(json_field "$second_write" ok)" = "True" ] && [ "$(churn_has_text "Stable stored: stable-ok-again")" = "1" ] && echo 1 || echo 0)" \
    "ok=$(json_field "$second_write" ok)"
act_target "$duplicate_button" click >/dev/null 2>&1
"$bin" wait --text "Layout: duplicated" --app "$app" --timeout 5000 >/dev/null 2>&1
output="$("$bin" set-value "$(target_ref "$stable")" poisoned 2>/dev/null)"
assert "ref with a live duplicate is AMBIGUOUS_TARGET without delivery" \
    "$([ "$(json_field "$output" ok)" = "False" ] && [ "$(json_field "$output" error.code)" = "AMBIGUOUS_TARGET" ] && [ "$(json_field "$output" error.disposition.delivery)" = "not_delivered" ] && echo 1 || echo 0)" \
    "code=$(json_field "$output" error.code) delivery=$(json_field "$output" error.disposition.delivery)"
batch_output="$(churn_batch_set_value "$(target_ref "$stable")" poisoned)"
assert "batch also reports AMBIGUOUS_TARGET without delivery" \
    "$([ "$(json_field "$batch_output" data.results.0.error.code)" = "AMBIGUOUS_TARGET" ] && [ "$(json_field "$batch_output" data.results.0.error.disposition.delivery)" = "not_delivered" ] && echo 1 || echo 0)" \
    "code=$(json_field "$batch_output" data.results.0.error.code)"
assert "original editor kept its value" "$(churn_has_text "Stable stored: stable-ok-again")" ""
assert "duplicate editor kept its value" "$(churn_has_text "Duplicate stored: Duplicate value")" ""
churn_stop

note "Complete app inventory gives a terminal unknown-app response"
run_timed "$bin" snapshot --app "AgentDeskMissing-$churn_pid" --timeout-ms 1500
assert "unknown app is APP_NOT_FOUND without delivery" \
    "$([ "$(json_field "$RUN_JSON" error.code)" = "APP_NOT_FOUND" ] && [ "$(json_field "$RUN_JSON" error.disposition.delivery)" = "not_delivered" ] && echo 1 || echo 0)" \
    "exit=$RUN_EXIT elapsed=$RUN_MS code=$(json_field "$RUN_JSON" error.code)"
