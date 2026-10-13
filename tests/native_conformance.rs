//! Golden observations produced by the actual native TPT engine, not by Rust.
//! No TPT installation or network access is needed to run these regressions.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use r3emu::bus::BusRequest;
use r3emu::machine::MachineConfig;
use serde::Deserialize;

type NativeSlot = (u16, u8, u8, u32, bool);

#[derive(Deserialize)]
struct Fixture {
    name: String,
    frames: usize,
    source_sha256: BTreeMap<String, String>,
    registers: [u32; 32],
    initial_ram: Vec<(usize, u32)>,
    final_ram_changes: Vec<(usize, u32)>,
    // Run length, PC, state, flags, incoming memory control lines, wait.
    slots: Vec<(usize, u16, u8, u8, u32, bool)>,
    has_screen_timeline: bool,
    screens: Vec<(usize, Vec<(usize, u8)>)>,
    final_pixels: Vec<(usize, u8)>,
}

#[derive(Deserialize)]
struct ReferenceCase {
    op: String,
    a: u16,
    b: u16,
    address: usize,
    expected: u32,
    flag: Option<char>,
    #[serde(default)]
    carry: u8,
}

fn check_independent_reference(name: &str, ram: &[u32]) {
    let (source, count) = match name {
        "arithmetic-matrix" => (
            include_str!("fixtures/reference/arithmetic-matrix.json"),
            96,
        ),
        "flags-matrix" => (include_str!("fixtures/reference/flags-matrix.json"), 64),
        "carry-matrix" => (include_str!("fixtures/reference/carry-matrix.json"), 160),
        _ => return,
    };
    let cases: Vec<ReferenceCase> = serde_json::from_str(source).unwrap();
    assert_eq!(cases.len(), count);
    for case in cases {
        // Wide integer calculations are deliberately independent of the VM ALU.
        let a = i64::from(case.a);
        let b = i64::from(case.b);
        let sa = i64::from(case.a as i16);
        let sb = i64::from(case.b as i16);
        let carry = i64::from(case.carry);
        let expected = if let Some(flag) = case.flag {
            let (unsigned, signed) = if matches!(case.op.as_str(), "sub" | "sbb") {
                (b - a - carry, sb - sa - carry)
            } else {
                (a + b + carry, sa + sb + carry)
            };
            let bit = match flag {
                'Z' => unsigned as u16 == 0,
                'S' => unsigned as u16 & 0x8000 != 0,
                'C' => !(0..=65535).contains(&unsigned),
                'O' => !(-32768..=32767).contains(&signed),
                _ => panic!("unknown reference flag"),
            };
            0x2000_0000 | u32::from(bit)
        } else if case.op == "exh" {
            u32::from(case.b) << 16 | 0x2000
        } else {
            let result = match case.op.as_str() {
                "add" => a + b,
                "sub" => b - a,
                "adc" => a + b + carry,
                "sbb" => b - a - carry,
                "and" => a & b,
                "or" => a | b,
                "xor" => a ^ b,
                "shl" => a << (b & 15),
                "shr" => a >> (b & 15),
                "mul" => a * b,
                "mulh" => (a * b) >> 16,
                "muls" => (sa * sb) >> 16,
                "mulx" => (a * sb) >> 16,
                _ => panic!("unknown reference operation"),
            };
            0x2000_0000 | u32::from(result as u16)
        };
        assert_eq!(
            expected, case.expected,
            "{name}: reference at {}",
            case.address
        );
        assert_eq!(
            ram[case.address], expected,
            "{name}: VM at {}",
            case.address
        );
    }
}

fn check_fixture(path: &Path) {
    let fixture: Fixture = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    let name = &fixture.name;
    assert!(fixture.frames > 0, "{name}: empty capture");
    assert!(fixture.source_sha256.contains_key("trace.csv"));
    assert!(fixture.source_sha256.values().all(|hash| hash.len() == 64));
    let mut vm = MachineConfig::showcase().build(None).unwrap();
    vm.initialize_registers(fixture.registers);
    for (address, value) in fixture.initial_ram {
        vm.bus.ram[address] = value;
    }
    let mut expected_ram = vm.bus.ram.clone();
    for (address, value) in fixture.final_ram_changes {
        expected_ram[address] = value;
    }
    let mut expected_slots: Vec<NativeSlot> = Vec::with_capacity(fixture.frames * 10);
    for (count, pc, state, flags, address, wait) in fixture.slots {
        assert!(count > 0 && count <= fixture.frames * 10);
        expected_slots.extend(std::iter::repeat_n(
            (pc, state, flags, address, wait),
            count,
        ));
    }
    assert_eq!(
        expected_slots.len(),
        fixture.frames * 10,
        "{name}: slot count"
    );
    let mut screen_changes = fixture.screens.into_iter().peekable();
    let mut expected_pixels = vec![0; 96 * 64];
    for frame in 0..=fixture.frames {
        if frame > 0 {
            let mut observed = Vec::with_capacity(10);
            vm.cycle_with_observer(|slot| observed.push(slot));
            for slot in observed {
                let mut control = match slot.request {
                    BusRequest::Read { .. } => 4,
                    BusRequest::Write { .. } => 1,
                };
                if vm.bus.is_external(slot.request.address()) {
                    control *= 2;
                }
                let actual = (
                    slot.pc,
                    slot.state as u8,
                    slot.flags,
                    u32::from(slot.request.address()) | control << 16,
                    slot.waited,
                );
                let expected = expected_slots[(frame - 1) * 10 + slot.core];
                assert_eq!(
                    actual, expected,
                    "{name}: frame {frame}, unit {}",
                    slot.core
                );
            }
        }
        if screen_changes.peek().is_some_and(|entry| entry.0 == frame) {
            let (_, changes) = screen_changes.next().unwrap();
            for (pixel, color) in changes {
                expected_pixels[pixel] = color;
            }
        }
        if fixture.has_screen_timeline {
            assert_eq!(
                vm.bus.terminal.pixbuf, expected_pixels,
                "{name}: display frame {frame}"
            );
        }
    }
    assert!(
        screen_changes.next().is_none(),
        "{name}: unconsumed display frames"
    );
    assert_eq!(vm.bus.ram, expected_ram, "{name}: final RAM");
    check_independent_reference(name, &vm.bus.ram);
    let mut final_pixels = vec![0; 96 * 64];
    for (pixel, color) in fixture.final_pixels {
        final_pixels[pixel] = color;
    }
    assert_eq!(
        vm.bus.terminal.pixbuf, final_pixels,
        "{name}: final display"
    );
}

#[test]
fn production_vm_matches_every_retained_native_slot_and_display_frame() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/r316");
    let mut paths: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    assert!(
        paths.len() >= 38,
        "native fixture set is missing or incomplete"
    );
    for path in paths {
        check_fixture(&path);
    }
}
