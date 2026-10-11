use flux_lib::utils::error_utils::home_dir_not_found;

#[test]
fn test_home_dir_not_found_message() {
  let err = home_dir_not_found();
  let msg = err.to_string();
  assert!(msg.contains("$HOME"));
  assert!(msg.contains("Solution"));
}
