use crate::alu::{sanitize_read, sanitize_write};
use crate::keyboard::Keyboard;
use crate::terminal::Terminal;

const IO_BASE: u16 = 0x9F80;

pub struct Bus {
    pub ram: Vec<u32>,
    mem_size: u16,
    mem_rows: u16,
    row_size: u16,
    mem_p2rows: u16,
    pub terminal: Terminal,
    pub keyboard: Keyboard,
    pub has_pixplot: bool,
}

impl Bus {
    pub fn new(rows: u8, row_size: u16, term_cols: u8, term_rows: u8, has_pixplot: bool) -> Self {
        let mem_rows = rows as u16;
        assert!(
            rows > 0 && row_size > 0,
            "memory dimensions must be nonzero"
        );
        let mem_size = mem_rows
            .checked_mul(row_size)
            .expect("memory exceeds 65535 words");
        let mem_p2rows = mem_rows.next_power_of_two();
        Bus {
            ram: vec![0; mem_size as usize],
            mem_size,
            mem_rows,
            row_size,
            mem_p2rows,
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
    fn map_addr(&self, addr: u16) -> (usize, bool) {
        let row_size = self.row_size;
        let block = addr / row_size;
        let offset = addr % row_size;
        let mem_rows = self.mem_rows;
        let mem_p2rows = self.mem_p2rows;

        let (row, writable) = if block < mem_rows {
            (block, true)
        } else if block < mem_p2rows {
            (mem_rows - 1, false)
        } else {
            let mirror = block % mem_p2rows;
            if mirror < mem_rows {
                (mirror, false)
            } else {
                (mem_rows - 1, false)
            }
        };

        let idx = row as usize * row_size as usize + offset as usize;
        (idx, writable)
    }

    #[inline(always)]
    pub fn read(&mut self, addr: u16) -> u32 {
        // I/O range reads
        if (IO_BASE..IO_BASE + 0x80).contains(&addr) {
            if addr == IO_BASE {
                return self.keyboard.get_key() as u32;
            }
            return 0;
        }
        // Internal memory mapping (including mirrors)
        let (idx, _) = self.map_addr(addr);
        sanitize_read(self.ram[idx])
    }

    #[inline(always)]
    pub fn write(&mut self, addr: u16, val: u32) {
        // I/O hooks
        if (IO_BASE..IO_BASE + 0x80).contains(&addr) {
            self.write_io(addr, sanitize_write(val));
            return;
        }

        let (idx, writable) = self.map_addr(addr);
        if writable {
            self.ram[idx] = sanitize_write(val);
        }
    }

    #[cold]
    fn write_io(&mut self, addr: u16, val: u32) {
        // Write hooks: check I/O address ranges
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "memory dimensions must be nonzero")]
    fn rejects_zero_rows() {
        Bus::new(0, 128, 1, 1, false);
    }

    #[test]
    #[should_panic(expected = "memory dimensions must be nonzero")]
    fn rejects_zero_row_size() {
        Bus::new(1, 0, 1, 1, false);
    }

    #[test]
    #[should_panic(expected = "memory exceeds 65535 words")]
    fn rejects_memory_size_overflow() {
        Bus::new(2, 32768, 1, 1, false);
    }

    #[test]
    fn test_memory_map_and_mirror() {
        let mut bus = Bus::new(3, 128, 1, 1, false);

        bus.write(0x0001, 0x12345678); // row 0
        bus.write(0x0080, 0x11112222); // row 1
        bus.write(0x0100, 0x33334444); // row 2

        // Block 3 maps to last row, read-only
        bus.write(0x0180, 0x55556666);
        assert_eq!(bus.read(0x0100), 0x33334444);
        assert_eq!(bus.read(0x0180), 0x33334444);

        // External mirrors (read-only)
        assert_eq!(bus.read(0x0201), 0x12345678); // mirror of row 0
        assert_eq!(bus.read(0x0280), 0x11112222); // mirror of row 1
        assert_eq!(bus.read(0x0300), 0x33334444); // mirror of row 2
    }

    #[test]
    fn test_phys_zero_sanitize() {
        let mut bus = Bus::new(1, 128, 1, 1, false);
        bus.write(0x0000, 0x00000000);
        assert_eq!(bus.read(0x0000), 0x20000000);

        bus.ram[0] = 0x00000000;
        assert_eq!(bus.read(0x0000), 0x0000001F);
    }
}
