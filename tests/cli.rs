use std::fs;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn run_words(words: &[u32], options: &[&str]) -> (Output, Vec<u32>) {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("r3emu-cli-{}-{stamp}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let path = root.join("input.bin");
    let bytes: Vec<_> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
    fs::write(&path, bytes).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_r3emu"))
        .current_dir(&root)
        .arg(&path)
        .args(["--headless", "--memdump", "--max-cycles", "16"])
        .args(options)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let ram = fs::read_to_string(root.join("memdump.bin"))
        .unwrap()
        .lines()
        .map(|line| line.parse().unwrap())
        .collect();
    fs::remove_dir_all(root).unwrap();
    (output, ram)
}

#[test]
fn showcase_profile_selects_ram_and_saved_register_contents() {
    let words = [0x7E0A_03E8, 0x000D_0000]; // st r31,1000; hlt
    let (_, generic) = run_words(&words, &[]);
    let (_, showcase) = run_words(&words, &["--profile", "showcase"]);
    assert_eq!(generic.len(), 8192);
    assert_eq!(showcase.len(), 2048);
    assert_eq!(generic[1000], 0x2000_0000);
    assert_eq!(showcase[1000], 0x2000_001F);
}

#[test]
fn explicit_mixed_layout_overrides_the_profile() {
    let words = [0x085F_0006, 0x0A5E_0006, 0x4A0A_03E8, 0x000D_0000];
    let (output, ram) = run_words(&words, &["--profile", "showcase", "--core-types", "fms"]);
    assert_eq!(ram[1000], 0x2000_001E);
    assert!(String::from_utf8_lossy(&output.stderr).contains("(halted)"));
    let (output, _) = run_words(&words, &["--core-types", "fff"]);
    assert!(String::from_utf8_lossy(&output.stderr).contains("frame limit"));
}

#[test]
fn bare_bus_is_an_explicit_alternative_to_unserved_access_waits() {
    let words = [0x4202_A004, 0x420A_03E8, 0x000D_0000, 0, 456];
    let (waiting, ram) = run_words(&words, &[]);
    assert_eq!(ram[1000], 0x2000_0000);
    assert!(String::from_utf8_lossy(&waiting.stderr).contains("frame limit"));
    let (completed, ram) = run_words(&words, &["--bare-bus"]);
    assert_eq!(ram[1000], 456);
    assert!(String::from_utf8_lossy(&completed.stderr).contains("(halted)"));
}

#[test]
fn keyboard_byte_survives_bus_waits_and_is_consumed_once() {
    let words = [
        0x4202_9F80,
        0x420A_03E8,
        0x4402_9F80,
        0x440A_03E9,
        0x000D_0000,
    ];
    let (_, ram) = run_words(
        &words,
        &[
            "--keyboard-input",
            "A",
            "--keyboard-input-delay-cycles",
            "0",
        ],
    );
    assert_eq!(ram[1000], 0x1000_0041);
    assert_eq!(ram[1001], 0x1000_0000);
}

#[test]
fn conflicting_topology_options_are_rejected() {
    for options in [
        ["--cores", "2", "--core-types", "ms"].as_slice(),
        ["--bare-bus", "--terminal-unit", "0"].as_slice(),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_r3emu"))
            .arg("tests/test0.bin")
            .args(options)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
    }
}

#[test]
fn rejects_invalid_dimensions() {
    for (flag, value) in [
        ("--memrows", "0"),
        ("--rowsize", "0"),
        ("--cores", "0"),
        ("--term-cols", "0"),
        ("--term-rows", "0"),
        ("--rowsize", "65535"),
        ("--term-cols", "30"),
        ("--term-rows", "30"),
        ("--core-types", ""),
        ("--core-types", "mx"),
        ("--terminal-unit", "10"),
        ("--profile", "unknown"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_r3emu"))
            .args(["tests/test0.bin", "--headless", flag, value])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{flag} {value}");
        assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
    }
}
