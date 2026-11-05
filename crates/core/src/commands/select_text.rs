use crate::{
    Action, AppError, PlatformAdapter, TextSelection,
    commands::helpers::{RefArgs, execute_ref_action_with_context},
    context::CommandContext,
};
use serde_json::Value;

pub struct SelectTextArgs {
    pub ref_id: String,
    pub selection: TextSelection,
    pub timeout_ms: Option<u64>,
}

pub fn execute(
    args: SelectTextArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    args.selection.validate()?;
    let request = context.request_base(Action::SelectText(args.selection));
    execute_ref_action_with_context(
        RefArgs {
            ref_id: args.ref_id,
            timeout_ms: args.timeout_ms,
        },
        adapter,
        request,
        context,
    )
}

#[cfg(test)]
#[path = "select_text_tests.rs"]
mod tests;
