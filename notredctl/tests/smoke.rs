//! Smoke tests for the `notredctl` binary.

#[test]
fn help_exits_zero() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_notredctl"))
        .arg("--help")
        .output()
        .expect("notredctl binary");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("notredctl"));
}
