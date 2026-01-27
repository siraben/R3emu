use clap::Parser;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Point;

use std::fs;
use std::io::Write;
use std::time::{Duration, Instant};

mod alu;
mod bus;
mod disassembler;
mod font;
mod keyboard;
mod terminal;
mod vm;

use bus::Bus;
use terminal::COLOR_TABLE;
use vm::{CoreType, Vm};

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

    /// Memory rows
    #[arg(long, default_value_t = 64)]
    memrows: u8,

    /// Number of cores
    #[arg(long, default_value_t = 10)]
    cores: u8,

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
    #[arg(long = "term-cols", default_value_t = 12)]
    term_cols: u8,

    /// Terminal character rows
    #[arg(long = "term-rows", default_value_t = 8)]
    term_rows: u8,

    /// Memory row size in words
    #[arg(long, default_value_t = 128)]
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

fn main() {
    let args = Args::parse();

    eprintln!("R3 emulator");
    eprintln!("Written by Justus Wolff in very late 2025-2026");
    eprintln!("With help from LBPHacker, to fix alot of arithmetic bugs, who also made the original R3");
    eprintln!("Also credit to siraben due to finding bugs and patching them by implementing haskell for the R3.");

    // Read input binary
    let file_bytes = match fs::read(&args.input) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Failed to read '{}': {}", args.input, e);
            std::process::exit(2);
        }
    };

    // Create VM
    let bus = Bus::new(
        args.memrows,
        args.rowsize,
        args.term_cols,
        args.term_rows,
        !args.no_pixplot,
    );
    let mem_size = bus.mem_size() as usize;

    let cores = vec![CoreType::M; args.cores as usize];
    let trace_size = if args.tracedump {
        Some(args.tracesize)
    } else {
        None
    };

    let mut vm = Vm::new(bus, cores, !args.no_smul, trace_size);
    vm.bus.terminal.mirror_stdout = args.stdout;

    // Load binary into memory (native byte order, matching C memcpy behavior)
    eprintln!("Reading into memory...");
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

    eprintln!(
        "Target fps: {}\nTarget ips: {}",
        args.targetfps,
        args.targetfps as u64 * args.cores as u64
    );
    eprintln!("Emulation started.");

    if args.headless {
        // Headless mode: run VM without SDL2
        while !vm.halted {
            vm.cycle();
        }
    } else {
        // SDL2 setup
        let sdl_context = sdl2::init().expect("Failed to init SDL2");
        let video = sdl_context.video().expect("Failed to init SDL2 video");

        let logical_w = 8 * args.term_cols as u32;
        let logical_h = 8 * args.term_rows as u32 + 64;

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

        while !vm.halted {
            let frame_start = Instant::now();

            vm.cycle();

            // Process events
            for event in event_pump.poll_iter() {
                match event {
                    Event::Quit { .. } => {
                        vm.halted = true;
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
            if frame >= args.updxframes {
                // Render memory visualization (below terminal area)
                let term_pix_h = 8 * args.term_rows as i32;
                for row in 0..args.memrows as i32 {
                    for col in 0..128i32 {
                        let addr = (col + 128 * row) as u16;
                        let val = vm.bus.read(addr);
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

    eprintln!("Emulation finished at IP '{}'", vm.ip);

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
