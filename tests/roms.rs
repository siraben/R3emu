use r3emu::alu::sanitize_write;
use r3emu::bus::Bus;
use r3emu::vm::{CoreType, Vm};

#[test]
fn bundled_batch_roms_halt() {
    for (name, bytes) in [
        ("bee_lzss", include_bytes!("bee_lzss.bin").as_slice()),
        ("bignum_pow2", include_bytes!("bignum_pow2.bin").as_slice()),
        ("triangle", include_bytes!("triangle.bin").as_slice()),
        ("test_fixes", include_bytes!("test_fixes.bin").as_slice()),
    ] {
        let mut vm = Vm::new(Bus::new(64, 128, 12, 8, true), vec![CoreType::M; 10], true, None);
        assert_eq!(bytes.len() % 4, 0);
        assert!(bytes.len() / 4 <= vm.bus.ram.len());
        for (cell, word) in vm.bus.ram.iter_mut().zip(bytes.chunks_exact(4)) {
            *cell = sanitize_write(u32::from_ne_bytes(word.try_into().unwrap()));
        }
        for _ in 0..5_000_000 {
            if vm.halted {
                break;
            }
            vm.cycle();
        }
        assert!(vm.halted, "{name} did not halt within the cycle budget");
    }
}
