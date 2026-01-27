use crate::alu::patch_word;
use crate::keyboard::Keyboard;
use crate::terminal::Terminal;

const IO_BASE: u16 = 0x9F80;

pub struct Bus {
    pub ram: Vec<u32>,
    mem_size: u16,
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
        let mem_size = rows as u16 * row_size;
        Bus {
            ram: vec![0; mem_size as usize],
            mem_size,
            terminal: Terminal::new(term_cols, term_rows),
            keyboard: Keyboard::new(),
            has_pixplot,
        }
    }

    #[inline(always)]
    pub fn mem_size(&self) -> u16 {
        self.mem_size
    }

    #[inline(always)]
    pub fn read(&mut self, addr: u16) -> u32 {
        let ms = self.mem_size;
        // Fast path: RAM access (vast majority of reads)
        if addr < ms {
            return patch_word(unsafe { *self.ram.get_unchecked(addr as usize) });
        }
        // Keyboard read hook
        if addr == IO_BASE {
            return self.keyboard.get_key() as u32;
        }
        0
    }

    #[inline(always)]
    pub fn write(&mut self, addr: u16, val: u32) {
        let ms = self.mem_size;
        // Fast path: RAM access (vast majority of writes)
        if addr < ms {
            unsafe { *self.ram.get_unchecked_mut(addr as usize) = patch_word(val) };
            return;
        }
        self.write_io(addr, val);
    }

    #[cold]
    fn write_io(&mut self, addr: u16, val: u32) {
        // Write hooks: check I/O address ranges (value is NOT patched for hooks)
        match addr {
            // Scroll print: 0x9F80..=0x9FBF (offsets 0x00..=0x3F)
            0x9F80..=0x9FBF => {
                self.terminal.scroll_print(val, addr - IO_BASE);
            }
            // Terminal registers
            0x9FC0 => {
                self.terminal.char0odd = val;
            }
            0x9FC1 => {
                self.terminal.char0even = val;
            }
            0x9FC2 => {
                self.terminal.hrange = val as u16;
            }
            0x9FC3 => {
                self.terminal.vrange = val as u16;
            }
            0x9FC4 => {
                self.terminal.cursor = val as u16;
            }
            0x9FC5 => {
                self.terminal.nlchar = val as u8;
            }
            0x9FC6 => {
                self.terminal.colors = val as u8;
            }
            0x9FC7 => {
                self.terminal.scrollmask = val;
            }
            // Pixel plot: 0x9FE0..=0x9FFF (offsets 0x60..=0x7F)
            0x9FE0..=0x9FFF => {
                if self.has_pixplot {
                    self.terminal.plot_pixel(val, addr - 0x9FE0);
                }
            }
            _ => {}
        }
    }
}
