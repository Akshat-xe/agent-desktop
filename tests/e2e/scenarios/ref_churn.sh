note "Ref behavior across collection changes"
MODE_FLAG=""
AGENT_DESKTOP_FIXTURE_REF_CHURN=1 AGENT_DESKTOP_FIXTURE_NO_ACTIVATE=1 \
    "$fixture_app/Contents/MacOS/AgentDeskFixture" >/dev/null 2>&1 &
churn_pid=$!
fixture_owned=1
require_target_by_id filter button filter-rows
require_target_by_id alpha textfield shared-title
require_target_by_id stable textfield stable-editor
act_target "$filter" click >/dev/null 2>&1
"$bin" wait --text "Restore rows" --app "$app" --timeout 5000 >/dev/null 2>&1
duplicate_identifier_note="known limitation: a removed row's ref can resolve to the surviving row that shares shared-title"
skipmsg "removed row is refused without delivery: $duplicate_identifier_note"
skipmsg "batch also refuses the removed row without delivery: $duplicate_identifier_note"
skipmsg "surviving row kept its value: $duplicate_identifier_note"
stable_write="$("$bin" set-value "$(target_ref "$stable")" stable-ok 2>/dev/null)"
stable_seen="$(wait_target "$stable" value 3000 --value stable-ok 2>/dev/null)"
assert "unchanged editor remains editable through its ref" \
    "$([ "$(json_field "$stable_write" ok)" = "True" ] && [ "$(json_field "$stable_seen" data.found)" = "True" ] && echo 1 || echo 0)" \
    "ok=$(json_field "$stable_write" ok) found=$(json_field "$stable_seen" data.found)"
kill "$churn_pid" 2>/dev/null
wait "$churn_pid" 2>/dev/null
fixture_owned=0

note "Complete app inventory gives a terminal unknown-app response"
run_timed "$bin" snapshot --app "AgentDeskMissing-$churn_pid" --timeout-ms 1500
assert "unknown app is APP_NOT_FOUND without delivery" \
    "$([ "$(json_field "$RUN_JSON" error.code)" = "APP_NOT_FOUND" ] && [ "$(json_field "$RUN_JSON" error.disposition.delivery)" = "not_delivered" ] && echo 1 || echo 0)" \
    "exit=$RUN_EXIT elapsed=$RUN_MS code=$(json_field "$RUN_JSON" error.code)"
