use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTextSelection {
    pub range: Range<usize>,
    pub utf16_range: Range<usize>,
    pub text: String,
}
