use r3emu::bus::{Bus, BusRequest, TerminalPort};
use r3emu::vm::{CoreType, ExecutionState, Vm};

fn vm(words: &[u32]) -> Vm {
    let mut vm = Vm::new(
        Bus::new(16, 128, 12, 8, true),
        vec![
            CoreType::M,
            CoreType::S,
            CoreType::F,
            CoreType::F,
            CoreType::F,
            CoreType::M,
            CoreType::S,
            CoreType::F,
            CoreType::F,
            CoreType::F,
        ],
        true,
        None,
    );
    vm.initialize_registers(std::array::from_fn(|i| 0x2000_0000 + i as u32));
    vm.bus.ram.fill(0x2000_0000);
    vm.bus.ram[..words.len()].copy_from_slice(words);
    vm
}

#[test]
fn dependent_memory_instructions_match_native_service_slots() {
    // mov r1,42; st r1,1000; ld r2,1000; add r3,r2,1; st r3,1001; hlt
    let mut vm = vm(&[
        0x4200_002A,
        0x420A_03E8,
        0x4402_03E8,
        0xC626_0001,
        0x460A_03E9,
        0x000D_0000,
    ]);
    let mut observed = Vec::new();
    vm.cycle_with_observer(|s| observed.push((s.pc, s.state as u8)));
    assert_eq!(
        observed,
        [
            (1, 1),
            (2, 4),
            (2, 1),
            (3, 2),
            (3, 1),
            (4, 1),
            (5, 4),
            (5, 1),
            (6, 8),
            (6, 8)
        ]
    );
    assert_eq!(vm.bus.ram[1000], 0x2000_002A);
    assert_eq!(vm.bus.ram[1001], 0x2000_002B);
}

#[test]
fn self_modifying_store_executes_the_already_fetched_instruction() {
    // mov r1,13; exhs r1,r0,r1; st r1,3; mov r2,7; st r2,1000; hlt
    let mut vm = vm(&[
        0x4200_000D,
        0x0203_0001,
        0x420A_0003,
        0x4400_0007,
        0x440A_03E8,
        0x000D_0000,
    ]);
    vm.cycle();
    assert!(vm.halted);
    assert_eq!(vm.bus.ram[3], 0x000D_2000);
    assert_eq!(vm.bus.ram[1000], 0x2000_0007);
}

#[test]
fn unserved_read_retries_without_register_or_pc_changes() {
    let mut vm = vm(&[0x4202_A000, 0x000D_0000]);
    for _ in 0..3 {
        vm.cycle();
        assert_eq!(vm.ip, 1);
        assert_eq!(vm.state(), ExecutionState::Load);
        assert_eq!(vm.registers()[1], 0x2000_0001);
    }
}

#[test]
fn io_write_waits_for_the_connected_unit_then_executes_the_cached_instruction() {
    let mut vm = vm(&[0x4200_0020, 0x420A_9FC2, 0x4400_0007, 0x000D_0000]);
    let mut observed = Vec::new();
    vm.cycle_with_observer(|s| observed.push((s.pc, s.state as u8, s.waited)));
    assert_eq!(
        observed[0..3],
        [(1, 1, false), (2, 4, false), (2, 1, false)]
    );
    assert!(observed[3..9].iter().all(|&s| s == (2, 1, true)));
    assert_eq!(observed[9], (3, 1, false));
    assert_eq!(vm.bus.terminal.hrange, 32);
    assert_eq!(vm.registers()[2], 0x2000_0007);
}

#[test]
fn halted_cpu_does_not_stop_the_terminal_pipeline() {
    let bus = Bus::new(16, 128, 1, 1, true);
    let mut vm = Vm::new(bus, vec![CoreType::M], true, None);
    vm.bus.terminal.colors = 15;
    vm.bus.ram[..3].copy_from_slice(&[0x4200_0041, 0x420A_9F85, 0x000D_0000]);
    for _ in 0..4 {
        vm.cycle();
    }
    assert!(vm.halted);
    assert!(vm.bus.has_pending_work());
    assert!(vm.bus.terminal.pixbuf.iter().all(|&pixel| pixel == 0));
    vm.cycle();
    assert!(vm.bus.terminal.pixbuf.iter().all(|&pixel| pixel == 0));
    vm.cycle();
    assert!(!vm.bus.has_pending_work());
    assert_eq!(vm.ip, 3);
    assert_eq!(
        vm.bus
            .terminal
            .pixbuf
            .iter()
            .filter(|&&pixel| pixel == 15)
            .count(),
        28
    );
}

#[test]
fn bare_bus_keeps_read_only_mirrors_instead_of_injecting_waits() {
    let bus = Bus::new(16, 128, 12, 8, true)
        .with_terminal_port(TerminalPort::Disconnected)
        .with_termination(false);
    let mut vm = Vm::new(bus, vec![CoreType::M], true, None);
    vm.bus.ram[0] = 0x4202_A003; // ld r1,0xa003 (mirror of RAM[3])
    vm.bus.ram[1] = 0x000D_0000;
    vm.bus.ram[3] = 123;
    vm.cycle();
    vm.cycle();
    assert_eq!(vm.registers()[1], 123);
    vm.cycle();
    assert!(vm.halted);
    assert_eq!(
        vm.bus.transact(
            0,
            1,
            BusRequest::Write {
                address: 0xA003,
                value: 99
            }
        ),
        r3emu::bus::BusResponse::Written
    );
    assert_eq!(vm.bus.ram[3], 123);
}
