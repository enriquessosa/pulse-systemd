#[test]
fn rejects_noninteractive_input_without_escape_sequences() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pulse"))
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("requires an interactive terminal"));
}
