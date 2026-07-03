//! Smoke tests for the `notred` binary.

#[test]
fn help_exits_zero() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_notred"))
        .arg("--help")
        .output()
        .expect("notred binary");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("notred"));
}
