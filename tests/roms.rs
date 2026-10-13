use r3emu::alu::sanitize_write;
use r3emu::bus::Bus;
use r3emu::vm::{CoreType, Vm};

fn load_rom(bytes: &[u8]) -> Vm {
    let mut vm = Vm::new(
        Bus::new(64, 128, 12, 8, true),
        vec![CoreType::M; 10],
        true,
        None,
    );
    assert_eq!(bytes.len() % 4, 0);
    assert!(bytes.len() / 4 <= vm.bus.ram.len());
    for (cell, word) in vm.bus.ram.iter_mut().zip(bytes.chunks_exact(4)) {
        *cell = sanitize_write(u32::from_ne_bytes(word.try_into().unwrap()));
    }
    vm
}

#[test]
fn bundled_batch_roms_halt() {
    for (name, bytes) in [
        ("bee_lzss", include_bytes!("bee_lzss.bin").as_slice()),
        ("test_fixes", include_bytes!("test_fixes.bin").as_slice()),
    ] {
        let mut vm = load_rom(bytes);
        for _ in 0..5_000_000 {
            if vm.halted {
                break;
            }
            vm.cycle();
        }
        assert!(vm.halted, "{name} did not halt within the cycle budget");
    }
}

#[test]
fn haskell_roms_boot_to_keyboard_wait_and_accept_input() {
    // These ROMs are interactive: they wait for a key before evaluating, then
    // return to the input loop. The addresses identify their three-instruction
    // keyboard polling loops, not a halt instruction.
    for (bytes, wait_ip) in [
        (include_bytes!("bignum_pow2.bin").as_slice(), 7740),
        (include_bytes!("triangle.bin").as_slice(), 7794),
    ] {
        let mut vm = load_rom(bytes);
        for _ in 0..100_000 {
            vm.cycle();
        }
        assert!(!vm.halted);
        assert!((wait_ip..wait_ip + 3).contains(&vm.ip));
        vm.bus.keyboard.queue_keypresses([b'\n']);
        // Input is serviced only on the last unit; the branch inspecting the
        // loaded byte may not execute until the following frame.
        let mut resumed = false;
        for _ in 0..4 {
            vm.cycle();
            if !(wait_ip..wait_ip + 3).contains(&vm.ip) {
                resumed = true;
                break;
            }
        }
        assert!(resumed, "keyboard input did not release the polling loop");
    }
}
