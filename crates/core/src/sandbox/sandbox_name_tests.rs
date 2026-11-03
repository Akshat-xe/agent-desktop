use super::validate_sandbox_name;

#[test]
fn accepts_lowercase_letters_digits_and_dashes_up_to_32_characters() {
    assert!(validate_sandbox_name("dev-1").is_ok());
    assert!(validate_sandbox_name(&"a".repeat(32)).is_ok());
    assert!(validate_sandbox_name(&"a".repeat(33)).is_err());
}

#[test]
fn rejects_uppercase_and_underscores_with_invalid_args() {
    let error = validate_sandbox_name("Dev_1").unwrap_err();
    assert_eq!(error.code(), "INVALID_ARGS");
    assert!(error.suggestion().is_some_and(|s| s.contains("dev-1")));
}
