use super::NotredBinError;

#[test]
fn config_error_display() {
    let err = NotredBinError::Config("bad toml".into());
    assert!(err.to_string().contains("bad toml"));
}
