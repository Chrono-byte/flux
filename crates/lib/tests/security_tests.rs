use flux_lib::utils::security;
use std::path::PathBuf;

#[test]
fn test_validate_dest_path_within_home() {
  let home = PathBuf::from("/home/user");
  let dest = PathBuf::from(".config/sway/config");

  let _ = security::validate_dest_path(&dest, &home);
}

#[test]
fn test_set_secure_permissions() -> Result<(), Box<dyn std::error::Error>> {
  use tempfile::tempdir;

  let dir = tempdir()?;
  let file_path = dir.path().join("test.txt");
  std::fs::write(&file_path, "test")?;

  let result = security::set_secure_permissions(&file_path);
  assert!(result.is_ok());
  Ok(())
}
