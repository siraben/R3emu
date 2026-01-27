use crate::alu::patch_word;
use crate::keyboard::Keyboard;
use crate::terminal::Terminal;

const IO_BASE: u16 = 0x9F80;

pub struct Bus {
    pub ram: Vec<u32>,
    rows: u8,
    row_size: u16,
    pub terminal: Terminal,
    pub keyboard: Keyboard,
    pub has_pixplot: bool,
}

impl Bus {
    pub fn new(
        rows: u8,
        row_size: u16,
        term_cols: u8,
        term_rows: u8,
        has_pixplot: bool,
    ) -> Self {
        let mem_size = rows as usize * row_size as usize;
        Bus {
            ram: vec![0; mem_size],
            rows,
            row_size,
            terminal: Terminal::new(term_cols, term_rows),
            keyboard: Keyboard::new(),
            has_pixplot,
        }
    }

    pub fn mem_size(&self) -> u16 {
        self.rows as u16 * self.row_size
    }

    pub fn read(&mut self, addr: u16) -> u32 {
        // Keyboard read hook
        if addr == IO_BASE {
            return self.keyboard.get_key() as u32;
        }
        // Regular memory
        if addr >= self.mem_size() {
            return 0;
        }
        patch_word(self.ram[addr as usize])
    }

    pub fn write(&mut self, addr: u16, val: u32) {
        // Write hooks: check I/O address ranges (value is NOT patched for hooks)
        match addr {
            // Scroll print: 0x9F80..=0x9FBF (offsets 0x00..=0x3F)
            0x9F80..=0x9FBF => {
                self.terminal.scroll_print(val, addr - IO_BASE);
                return;
            }
            // Terminal registers
            0x9FC0 => {
                self.terminal.char0odd = val;
                return;
            }
            0x9FC1 => {
                self.terminal.char0even = val;
                return;
            }
            0x9FC2 => {
                self.terminal.hrange = val as u16;
                return;
            }
            0x9FC3 => {
                self.terminal.vrange = val as u16;
                return;
            }
            0x9FC4 => {
                self.terminal.cursor = val as u16;
                return;
            }
            0x9FC5 => {
                self.terminal.nlchar = val as u8;
                return;
            }
            0x9FC6 => {
                self.terminal.colors = val as u8;
                return;
            }
            0x9FC7 => {
                self.terminal.scrollmask = val;
                return;
            }
            // Pixel plot: 0x9FE0..=0x9FFF (offsets 0x60..=0x7F)
            0x9FE0..=0x9FFF => {
                if self.has_pixplot {
                    self.terminal.plot_pixel(val, addr - 0x9FE0);
                }
                return;
            }
            _ => {}
        }
        // Regular memory write (patchword applied)
        if addr >= self.mem_size() {
            return;
        }
        self.ram[addr as usize] = patch_word(val);
    }
}
