//! Trace the production VM through its public observer API.
//! Usage: conformance_trace ROM OUTPUT FRAMES [CORE_TYPES] [RAM_ROWS]
//! Initial registers and terminal defaults match the pinned R316 showcase save.

use std::env;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::Path;

use r3emu::bus::BusRequest;
use r3emu::machine::{parse_cores, MachineConfig};
use r3emu::terminal::COLOR_TABLE;
use r3emu::vm::Vm;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if !(4..=6).contains(&args.len()) {
        return Err("usage: conformance_trace ROM OUTPUT FRAMES [CORE_TYPES] [RAM_ROWS]".into());
    }
    let frames: usize = args[3].parse()?;
    let layout = args.get(4).map(String::as_str).unwrap_or("msfffmsfff");
    let rows: u8 = args.get(5).map(String::as_str).unwrap_or("16").parse()?;
    let config = MachineConfig {
        memory_rows: rows,
        cores: parse_cores(layout)?,
        ..MachineConfig::showcase()
    };
    let mut vm = config.build(None)?;
    let rom = fs::read(&args[1])?;
    if rom.len() % 4 != 0 || rom.len() > vm.bus.ram.len() * 4 {
        return Err("ROM is not a sequence of words fitting the configured RAM".into());
    }
    vm.bus.ram.fill(0x2000_0000);
    for (index, bytes) in rom.chunks_exact(4).enumerate() {
        vm.bus.ram[index] = r3emu::alu::sanitize_write(u32::from_le_bytes(bytes.try_into()?));
    }
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let mut slots = BufWriter::new(File::create(out.join("slots.csv"))?);
    let mut screens = BufWriter::new(File::create(out.join("screen-trace.rgb"))?);
    writeln!(slots, "frame,core,pc,state,flags,ram_addr,io_state")?;
    let external_start = u32::from(rows).next_power_of_two() * 128;
    write_screen(&vm, &mut screens)?;
    for frame in 1..=frames {
        let mut observations = Vec::with_capacity(vm.cores.len());
        vm.cycle_with_observer(|slot| observations.push(slot));
        for slot in observations {
            let address = u32::from(slot.request.address());
            let internal_control = match slot.request {
                BusRequest::Read { .. } => 4,
                BusRequest::Write { .. } => 1,
            };
            let control = if address >= external_start {
                internal_control * 2
            } else {
                internal_control
            };
            writeln!(
                slots,
                "{frame},{},{},{},{},{},{}",
                slot.core,
                slot.pc,
                slot.state as u8,
                slot.flags,
                address | (control << 16),
                u8::from(slot.waited)
            )?;
        }
        write_screen(&vm, &mut screens)?;
    }
    let mut ram = BufWriter::new(File::create(out.join("final.bin"))?);
    for word in &vm.bus.ram {
        ram.write_all(&word.to_le_bytes())?;
    }
    let mut screen = BufWriter::new(File::create(out.join("screen-final.ppm"))?);
    writeln!(screen, "P6\n96 64\n255")?;
    write_screen(&vm, &mut screen)?;
    Ok(())
}

fn write_screen(vm: &Vm, output: &mut impl Write) -> io::Result<()> {
    for &pixel in &vm.bus.terminal.pixbuf {
        output.write_all(&COLOR_TABLE[usize::from(pixel)])?;
    }
    Ok(())
}
