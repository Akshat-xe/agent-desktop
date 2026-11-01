//! Bounded reads of the C strings and arrays a caller-supplied ref entry carries. Each length
//! is checked against the published `AD_MAX_*` constant and the pointer before memory is touched.

use crate::convert::string::required_adapter_string;
use agent_desktop_core::{AdapterError, ErrorCode};

pub(crate) fn check_array_len(
    len: usize,
    is_null: bool,
    field: &str,
    constant: &str,
    max: usize,
) -> Result<(), AdapterError> {
    if len > max {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            format!("{field} count {len} exceeds {constant} ({max})"),
        ));
    }
    if is_null {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            format!("{field} count is nonzero but pointer is null"),
        ));
    }
    Ok(())
}

pub(crate) unsafe fn string_array(
    ptr: *const *const std::os::raw::c_char,
    len: usize,
    field: &str,
    constant: &str,
    max: usize,
) -> Result<Vec<String>, AdapterError> {
    if len == 0 {
        return Ok(Vec::new());
    }
    check_array_len(len, ptr.is_null(), field, constant, max)?;
    let items = unsafe { std::slice::from_raw_parts(ptr, len) };
    items
        .iter()
        .enumerate()
        .map(|(index, item)| required_adapter_string(*item, &format!("{field}[{index}]")))
        .collect()
}

pub(crate) unsafe fn ref_path(
    ptr: *const u32,
    len: usize,
) -> Result<smallvec::SmallVec<[usize; 8]>, AdapterError> {
    if len == 0 {
        return Ok(smallvec::SmallVec::new());
    }
    check_array_len(
        len,
        ptr.is_null(),
        "path",
        "AD_MAX_REF_PATH_DEPTH",
        crate::types::ref_entry::AD_MAX_REF_PATH_DEPTH,
    )?;
    let mut path = smallvec::SmallVec::new();
    path.extend(
        unsafe { std::slice::from_raw_parts(ptr, len) }
            .iter()
            .map(|item| *item as usize),
    );
    Ok(path)
}
