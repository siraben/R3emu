use criterion::{criterion_group, criterion_main, Criterion};
use r3emu::bus::Bus;
use r3emu::vm::{CoreType, Vm};
use std::fs;

fn load_vm() -> Vm {
    let file_bytes = fs::read("tests/bee_lzss.bin").expect("failed to read tests/bee_lzss.bin");

    let bus = Bus::new(64, 128, 12, 8, true);
    let mem_size = bus.mem_size() as usize;

    let cores = vec![CoreType::M; 10];
    let mut vm = Vm::new(bus, cores, true, None);

    let words_in_file = file_bytes.len() / 4;
    let words_to_load = words_in_file.min(mem_size);
    for i in 0..words_to_load {
        let offset = i * 4;
        vm.bus.ram[i] = u32::from_ne_bytes([
            file_bytes[offset],
            file_bytes[offset + 1],
            file_bytes[offset + 2],
            file_bytes[offset + 3],
        ]);
    }

    vm
}

fn bench_bee_lzss(c: &mut Criterion) {
    c.bench_function("bee_lzss_headless", |b| {
        b.iter_with_setup(load_vm, |mut vm| {
            while !vm.halted {
                vm.cycle();
            }
        });
    });
}

criterion_group!(benches, bench_bee_lzss);
criterion_main!(benches);
