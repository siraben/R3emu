use std::process::Command;

#[test]
fn rejects_partial_and_oversized_roms() {
    let path = std::env::temp_dir().join(format!("r3emu-invalid-{}.bin", std::process::id()));
    for bytes in [vec![0; 3], vec![], vec![0; 516]] {
        std::fs::write(&path, bytes).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_r3emu"))
            .arg(&path)
            .args(["--headless", "--memrows", "1"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
    }
    std::fs::remove_file(path).unwrap();
}
