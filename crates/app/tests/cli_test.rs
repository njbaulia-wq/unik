use std::process::Command;

#[test]
fn test_cli_help_flag() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_fluxcut"));
    let output = cmd
        .arg("--help")
        .output()
        .expect("Failed to execute fluxcut");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("FluxCut"));
    assert!(stdout.contains("--diagnostics"));
}

#[test]
fn test_cli_version_flag() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_fluxcut"));
    let output = cmd
        .arg("--version")
        .output()
        .expect("Failed to execute fluxcut");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("FluxCut"));
}

#[test]
fn test_cli_diagnostics_flag() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_fluxcut"));
    let output = cmd
        .arg("--diagnostics")
        .output()
        .expect("Failed to execute fluxcut");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("FluxCut Diagnostic Report"));
    assert!(stdout.contains("Hardware & GPU Capabilities"));
}
