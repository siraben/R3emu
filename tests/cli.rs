use std::process::Command;

#[test]
fn rejects_invalid_dimensions() {
    for (flag, value) in [
        ("--memrows", "0"),
        ("--rowsize", "0"),
        ("--cores", "0"),
        ("--term-cols", "0"),
        ("--term-rows", "0"),
        ("--rowsize", "65535"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_r3emu"))
            .args(["tests/test0.bin", "--headless", flag, value])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{flag} {value}");
        assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
    }
}
