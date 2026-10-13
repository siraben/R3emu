use clap::{error::ErrorKind, CommandFactory, Parser, ValueEnum};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Point;

use std::fs;
use std::io::Write;
use std::time::{Duration, Instant};

use r3emu::alu::sanitize_write;
use r3emu::bus::TerminalPort;
use r3emu::disassembler;
use r3emu::machine::{parse_cores, MachineConfig};
use r3emu::terminal::COLOR_TABLE;
use r3emu::vm::Vm;

#[derive(Clone, Copy, Default, ValueEnum)]
enum Profile {
    #[default]
    Generic,
    Showcase,
}

#[derive(Parser)]
#[command(
    name = "r3emu",
    about = "R3 emulator",
    long_about = "R3 emulator\n\
        Written by Justus Wolff in very late 2025-2026\n\
        With help from LBPHacker, to fix alot of arithmetic bugs, who also made the original R3\n\
        Also credit to siraben due to finding bugs and patching them by implementing haskell for the R3."
)]
struct Args {
    /// Input binary file
    input: String,

    /// Machine defaults: generic (64 rows, ten M units) or save 3236906
    #[arg(long, value_enum, default_value = "generic")]
    profile: Profile,

    /// Override memory rows (generic: 64; showcase: 16)
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..))]
    memrows: Option<u8>,

    /// Override unit count with an all-M layout (default: 10)
    #[arg(long, conflicts_with = "core_types", value_parser = clap::value_parser!(u8).range(1..))]
    cores: Option<u8>,

    /// Explicit spatial layout, e.g. msfffmsfff (case-insensitive)
    #[arg(long)]
    core_types: Option<String>,

    /// Zero-based terminal connector unit (default: last unit)
    #[arg(long, conflicts_with = "bare_bus")]
    terminal_unit: Option<usize>,

    /// Disconnect peripherals and dummy terminators; expose bare RAM mirrors
    #[arg(long)]
    bare_bus: bool,

    /// Stop after this many frames, even if the program has not halted
    #[arg(long)]
    max_cycles: Option<u64>,

    /// Target FPS
    #[arg(long, default_value_t = 60)]
    targetfps: u32,

    /// SDL update interval in frames
    #[arg(long, default_value_t = 10)]
    updxframes: u64,

    /// Trace buffer size
    #[arg(long, default_value_t = 100000)]
    tracesize: usize,

    /// Terminal character columns
    #[arg(long = "term-cols", default_value_t = 12, value_parser = clap::value_parser!(u8).range(1..))]
    term_cols: u8,

    /// Terminal character rows
    #[arg(long = "term-rows", default_value_t = 8, value_parser = clap::value_parser!(u8).range(1..))]
    term_rows: u8,

    /// Memory row size in words
    #[arg(long, default_value_t = 128, value_parser = clap::value_parser!(u16).range(1..))]
    rowsize: u16,

    /// Dump memory after emulation
    #[arg(long)]
    memdump: bool,

    /// Enable execution trace dump
    #[arg(long)]
    tracedump: bool,

    /// Disable FPS limiter
    #[arg(long = "no-fpslimiter")]
    no_fpslimiter: bool,

    /// Disallow S-type core multiplication
    #[arg(long = "no-smul")]
    no_smul: bool,

    /// Disable pixel plotting
    #[arg(long = "no-pixplot")]
    no_pixplot: bool,

    /// Mirror terminal output to stdout
    #[arg(long)]
    stdout: bool,

    /// Run without UI
    #[arg(long)]
    headless: bool,

    /// Queue literal keyboard input bytes after the configured delay
    #[arg(long = "keyboard-input")]
    keyboard_input: Option<String>,

    /// Queue keyboard input bytes from a file after the configured delay
    #[arg(long = "keyboard-input-file")]
    keyboard_input_file: Option<String>,

    /// Cycles to wait before scripted keyboard input becomes available
    #[arg(long = "keyboard-input-delay-cycles", default_value_t = 1)]
    keyboard_input_delay_cycles: u64,
}

fn calc_mem_color(value: u32) -> [u8; 3] {
    let wl = (value as u64) | 0x20000000;
    let mut r: u64 = 0;
    let mut g: u64 = 0;
    let mut b: u64 = 0;
    let a: u64 = 127;

    for x in 0u64..12 {
        r += (wl >> (x + 18)) & 1;
        b += (wl >> x) & 1;
    }
    for x in 0u64..12 {
        g += (wl >> (x + 9)) & 1;
    }

    let scale = 624 / (r + g + b + 1);
    r = a * (r * scale).min(255) / 0xFF;
    g = a * (g * scale).min(255) / 0xFF;
    b = a * (b * scale).min(255) / 0xFF;

    [r as u8, g as u8, b as u8]
}

fn configuration(args: &Args) -> Result<MachineConfig, &'static str> {
    let mut config = match args.profile {
        Profile::Generic => MachineConfig::default(),
        Profile::Showcase => MachineConfig::showcase(),
    };
    if let Some(rows) = args.memrows {
        config.memory_rows = rows;
    }
    if let Some(count) = args.cores {
        config.cores = vec![r3emu::vm::CoreType::M; usize::from(count)];
    }
    if let Some(layout) = &args.core_types {
        config.cores = parse_cores(layout)?;
    }
    if let Some(unit) = args.terminal_unit {
        config.terminal_port = TerminalPort::Unit(unit);
    }
    if args.bare_bus {
        config.terminal_port = TerminalPort::Disconnected;
        config.terminate_unmapped = false;
    }
    config.row_size = args.rowsize;
    config.terminal_columns = args.term_cols;
    config.terminal_rows = args.term_rows;
    config.pixel_plotter = !args.no_pixplot;
    config.secondary_multiply = !args.no_smul;
    config.validate()?;
    Ok(config)
}

fn should_run(vm: &Vm, frames: u64, limit: Option<u64>) -> bool {
    (!vm.halted || vm.bus.has_pending_work()) && limit.is_none_or(|limit| frames < limit)
}

fn main() {
    let args = Args::parse();
    let config = configuration(&args).unwrap_or_else(|error| {
        Args::command()
            .error(ErrorKind::ValueValidation, error)
            .exit()
    });

    eprintln!("R3 emulator");
    eprintln!("Written by Justus Wolff in very late 2025-2026");
    eprintln!(
        "With help from LBPHacker, to fix alot of arithmetic bugs, who also made the original R3"
    );
    eprintln!("Also credit to siraben due to finding bugs and patching them by implementing haskell for the R3.");

    // Read input binary
    let file_bytes = match fs::read(&args.input) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Failed to read '{}': {}", args.input, e);
            std::process::exit(2);
        }
    };

    if file_bytes.is_empty() || file_bytes.len() % 4 != 0 {
        eprintln!("Input must contain a nonempty sequence of 32-bit words");
        std::process::exit(2);
    }
    if file_bytes.len() / 4 > usize::from(config.memory_rows) * usize::from(config.row_size) {
        eprintln!("Input exceeds configured RAM capacity");
        std::process::exit(2);
    }

    let trace_size = args.tracedump.then_some(args.tracesize);
    let mut vm = config
        .build(trace_size)
        .expect("configuration was validated");
    let mem_size = usize::from(vm.bus.mem_size());
    vm.bus.terminal.mirror_stdout = args.stdout;
    let mut scripted_keyboard_input = Vec::new();
    if let Some(input) = args.keyboard_input.as_ref() {
        scripted_keyboard_input.extend(input.bytes());
    }
    if let Some(path) = args.keyboard_input_file.as_ref() {
        match fs::read(path) {
            Ok(bytes) => scripted_keyboard_input.extend(bytes),
            Err(e) => {
                eprintln!("Failed to read keyboard input file '{}': {}", path, e);
                std::process::exit(2);
            }
        }
    }
    let mut keyboard_input_queued = scripted_keyboard_input.is_empty();
    let mut elapsed_cycles = 0u64;

    // ROM words have a fixed little-endian representation, independent of host.
    eprintln!("Reading into memory...");
    for (cell, bytes) in vm.bus.ram.iter_mut().zip(file_bytes.chunks_exact(4)) {
        *cell = sanitize_write(u32::from_le_bytes(
            bytes.try_into().expect("four-byte chunk"),
        ));
    }

    eprintln!(
        "Target fps: {}\nTarget execution-unit slots/s: {}",
        args.targetfps,
        u64::from(args.targetfps) * config.cores.len() as u64
    );
    eprintln!("Emulation started.");

    if args.headless {
        // Drain pending display stages after HLT, just as the native engine does.
        while should_run(&vm, elapsed_cycles, args.max_cycles) {
            if !keyboard_input_queued && elapsed_cycles >= args.keyboard_input_delay_cycles {
                vm.bus
                    .keyboard
                    .queue_keypresses(scripted_keyboard_input.iter().copied());
                keyboard_input_queued = true;
            }
            vm.cycle();
            elapsed_cycles += 1;
        }
    } else {
        // SDL2 setup
        let sdl_context = sdl2::init().expect("Failed to init SDL2");
        let video = sdl_context.video().expect("Failed to init SDL2 video");

        let logical_w = (8 * args.term_cols as u32).max(args.rowsize as u32);
        let logical_h = 8 * args.term_rows as u32 + u32::from(config.memory_rows);

        let window = video
            .window("R3 Emulator", 300, 500)
            .position_centered()
            .build()
            .expect("Failed to create window");

        let mut canvas = window
            .into_canvas()
            .build()
            .expect("Failed to create canvas");

        canvas
            .set_logical_size(logical_w, logical_h)
            .expect("Failed to set logical size");

        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();
        canvas.present();

        let mut event_pump = sdl_context.event_pump().expect("Failed to get event pump");

        let frame_duration = if !args.no_fpslimiter && args.targetfps > 0 {
            Some(Duration::from_secs_f64(1.0 / args.targetfps as f64))
        } else {
            None
        };

        let mut frame: u64 = 0;
        let mut quit = false;

        while !quit && should_run(&vm, elapsed_cycles, args.max_cycles) {
            let frame_start = Instant::now();

            if !keyboard_input_queued && elapsed_cycles >= args.keyboard_input_delay_cycles {
                vm.bus
                    .keyboard
                    .queue_keypresses(scripted_keyboard_input.iter().copied());
                keyboard_input_queued = true;
            }
            vm.cycle();
            elapsed_cycles += 1;

            // Process events
            for event in event_pump.poll_iter() {
                match event {
                    Event::Quit { .. } => {
                        quit = true;
                    }
                    Event::KeyDown {
                        keycode: Some(key), ..
                    } => {
                        let k: i32 = key.into();
                        let a: i32 = Keycode::A.into();
                        let z: i32 = Keycode::Z.into();
                        let n0: i32 = Keycode::Num0.into();
                        let n9: i32 = Keycode::Num9.into();
                        let ch: u8 = if k >= a && k <= z {
                            b'a' + (k - a) as u8
                        } else if k >= n0 && k <= n9 {
                            b'0' + (k - n0) as u8
                        } else {
                            match key {
                                Keycode::Return => b'\n',
                                Keycode::Backspace => 8,
                                Keycode::Tab => b'\t',
                                Keycode::Space => b' ',
                                Keycode::Escape => 27,
                                _ => 0,
                            }
                        };
                        if ch != 0 {
                            vm.bus.keyboard.register_keypress(ch);
                        }
                    }
                    _ => {}
                }
            }

            frame += 1;
            if frame >= args.updxframes || quit || !should_run(&vm, elapsed_cycles, args.max_cycles)
            {
                // Render memory visualization (below terminal area)
                let term_pix_h = 8 * args.term_rows as i32;
                for row in 0..i32::from(config.memory_rows) {
                    for col in 0..args.rowsize as i32 {
                        let addr = (col + args.rowsize as i32 * row) as usize;
                        // Rendering must not trigger memory-mapped I/O reads.
                        let val = vm.bus.ram[addr];
                        let color = calc_mem_color(val);
                        canvas.set_draw_color(Color::RGB(color[0], color[1], color[2]));
                        let _ = canvas.draw_point(Point::new(col, row + term_pix_h));
                    }
                }

                // Render terminal pixel buffer
                let term_w = vm.bus.terminal.pixel_width() as i32;
                let term_h = vm.bus.terminal.pixel_height() as i32;
                let stride = term_w as usize;
                for y in 0..term_h {
                    for x in 0..term_w {
                        let idx = x as usize + y as usize * stride;
                        let ci = if idx < vm.bus.terminal.pixbuf.len() {
                            vm.bus.terminal.pixbuf[idx] as usize & 0xF
                        } else {
                            0
                        };
                        let rgb = &COLOR_TABLE[ci];
                        canvas.set_draw_color(Color::RGB(rgb[0], rgb[1], rgb[2]));
                        let _ = canvas.draw_point(Point::new(x, y));
                    }
                }

                frame = 0;
                canvas.present();
            }

            // FPS limiting
            if let Some(dur) = frame_duration {
                let elapsed = frame_start.elapsed();
                if elapsed < dur {
                    std::thread::sleep(dur - elapsed);
                }
            }
        }
    }

    eprintln!(
        "Emulation stopped after {elapsed_cycles} frames at IP '{}'{}",
        vm.ip,
        if vm.halted {
            " (halted)"
        } else {
            " (interrupted or frame limit)"
        }
    );

    // Memory dump
    if args.memdump {
        eprintln!("Dumping memory...");
        if let Ok(mut f) = fs::File::create("memdump.bin") {
            for i in 0..mem_size {
                let _ = writeln!(f, "{}", vm.bus.ram[i]);
            }
        }

        eprintln!("Dumping disassembled memory...");
        if let Ok(mut f) = fs::File::create("memdumpdisasm.asm") {
            let _ = writeln!(f, "%include \"common\"");
            for i in 0..mem_size {
                let _ = writeln!(f, "{}", disassembler::disassemble(vm.bus.ram[i]));
            }
        }
    }

    // Trace dump
    if let Some(ref trace) = vm.trace {
        eprintln!("Dumping trace...");
        if let Ok(mut f) = fs::File::create("tracedump.bin") {
            for entry in &trace.entries {
                let _ = writeln!(f, "{}", entry.instruction);
            }
        }

        eprintln!("Dumping disassembled trace...");
        if let Ok(mut f) = fs::File::create("tracedumpdisasm.asm") {
            let _ = writeln!(f, "%include \"common\"");
            for entry in &trace.entries {
                let disasm = disassembler::disassemble(entry.instruction);
                let _ = writeln!(
                    f,
                    "{}: {}    {}/{}/{}",
                    entry.addr, disasm, entry.ops[0], entry.ops[1], entry.ops[2]
                );
            }
        }
    }
}
