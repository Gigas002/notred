use super::IpcError;

#[test]
fn server_error_display() {
    let err = IpcError::ServerError("not found".into());
    assert_eq!(err.to_string(), "server error: not found");
}
