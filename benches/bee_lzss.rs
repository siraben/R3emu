use criterion::{criterion_group, criterion_main, Criterion};
use r3emu::alu::sanitize_write;
use r3emu::machine::MachineConfig;
use r3emu::vm::Vm;

fn load_vm() -> Vm {
    let bytes = include_bytes!("../tests/bee_lzss.bin");
    let mut vm = MachineConfig::default().build(None).unwrap();
    assert_eq!(bytes.len() % 4, 0);
    assert!(bytes.len() / 4 <= vm.bus.ram.len());
    for (cell, word) in vm.bus.ram.iter_mut().zip(bytes.chunks_exact(4)) {
        *cell = sanitize_write(u32::from_le_bytes(word.try_into().unwrap()));
    }
    vm
}

fn bench_bee_lzss(c: &mut Criterion) {
    c.bench_function("bee_lzss_headless", |b| {
        b.iter_with_setup(load_vm, |mut vm| {
            while !vm.halted || vm.bus.has_pending_work() {
                vm.cycle();
            }
            std::hint::black_box(vm);
        });
    });
}

criterion_group!(benches, bench_bee_lzss);
criterion_main!(benches);
