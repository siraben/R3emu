use std::collections::VecDeque;

use crate::alu::{sanitize_read, sanitize_write};
use crate::keyboard::Keyboard;
use crate::terminal::{RenderOperation, Terminal};

const IO_BASE: u16 = 0x9F80;

/// Memory control lines entering an execution unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BusRequest {
    Read { address: u16 },
    Write { address: u16, value: u32 },
}

impl BusRequest {
    pub fn address(self) -> u16 {
        match self {
            Self::Read { address } | Self::Write { address, .. } => address,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BusResponse {
    Read(u32),
    Written,
    Wait,
}

/// A peripheral is connected to a physical unit, not to every CPU instruction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TerminalPort {
    #[default]
    Last,
    Unit(usize),
    Disconnected,
}

struct ScheduledRender {
    frame: u64,
    operation: RenderOperation,
}

pub struct Bus {
    pub ram: Vec<u32>,
    mem_size: u16,
    mem_rows: u16,
    row_size: u16,
    mem_p2rows: u16,
    pub terminal: Terminal,
    pub keyboard: Keyboard,
    pub has_pixplot: bool,
    terminal_port: TerminalPort,
    terminate_unmapped: bool,
    frame: u64,
    deferred_scroll: Option<(u32, u16)>,
    terminal_busy: bool,
    pending_render: VecDeque<ScheduledRender>,
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
            terminal_port: TerminalPort::Last,
            terminate_unmapped: true,
            frame: 0,
            deferred_scroll: None,
            terminal_busy: false,
            pending_render: VecDeque::new(),
        }
    }

    /// Select the unit carrying the terminal's bus connector.
    pub fn with_terminal_port(mut self, port: TerminalPort) -> Self {
        self.terminal_port = port;
        self
    }

    /// A conventional dummy terminator stalls unserved external accesses. Disable
    /// it (and disconnect the terminal) to model the bare internal-memory mirrors.
    pub fn with_termination(mut self, enabled: bool) -> Self {
        self.terminate_unmapped = enabled;
        self
    }

    pub(crate) fn validate_topology(&self, units: usize) {
        if let TerminalPort::Unit(unit) = self.terminal_port {
            assert!(unit < units, "terminal unit is outside the CPU layout");
        }
    }

    /// Advance controller work at the start of a native simulation frame.
    pub(crate) fn begin_frame(&mut self) {
        self.frame += 1;
        self.terminal_busy = self.deferred_scroll.is_some();
        if let Some((value, offset)) = self.deferred_scroll.take() {
            self.schedule_scroll(value, offset);
        }
    }

    /// Display stages keep progressing even after the CPU executes HLT.
    pub(crate) fn end_frame(&mut self) {
        for _ in 0..self.pending_render.len() {
            let render = self.pending_render.pop_front().expect("known queue length");
            if render.frame <= self.frame {
                self.terminal.render(render.operation);
            } else {
                self.pending_render.push_back(render);
            }
        }
    }

    pub fn has_pending_work(&self) -> bool {
        self.deferred_scroll.is_some() || !self.pending_render.is_empty()
    }

    fn schedule_scroll(&mut self, value: u32, offset: u16) {
        let (operation, retry) = self.terminal.plan_scroll_print(value, offset);
        if let Some(operation) = operation {
            // Measured in native AFTERSIM observations: glyph/scroll changes
            // reach the screen two frames after their bus acknowledgement.
            self.pending_render.push_back(ScheduledRender {
                frame: self.frame + 2,
                operation,
            });
        }
        if retry {
            self.deferred_scroll = Some((value, offset));
        }
    }

    fn accept_terminal_write(&mut self, address: u16, value: u32) {
        match address {
            0x9F80..=0x9FBF => self.schedule_scroll(value, address - IO_BASE),
            0x9FE0..=0x9FFF if self.has_pixplot => {
                // Native overlap probes put pixel and glyph visibility two
                // frames after acknowledgement, including a deferred print.
                self.pending_render.push_back(ScheduledRender {
                    frame: self.frame + 2,
                    operation: RenderOperation::Pixel {
                        column: value as u8,
                        row: (value >> 8) as u8,
                        color: (address & 15) as u8,
                    },
                });
            }
            _ => self.write_io(address, value),
        }
    }

    pub fn is_external(&self, address: u16) -> bool {
        u32::from(address) >= u32::from(self.mem_p2rows) * u32::from(self.row_size)
    }

    /// Service one unit's incoming control lines. Wait leaves the transaction
    /// untouched so the following unit can retry, without duplicated effects.
    pub fn transact(&mut self, unit: usize, units: usize, request: BusRequest) -> BusResponse {
        let address = request.address();
        let connected = match self.terminal_port {
            TerminalPort::Last => unit + 1 == units,
            TerminalPort::Unit(port) => unit == port,
            TerminalPort::Disconnected => false,
        };
        let external = self.is_external(address);
        if external && connected && (IO_BASE..IO_BASE + 0x80).contains(&address) {
            return match request {
                BusRequest::Read { .. } => BusResponse::Read(self.read(address)),
                BusRequest::Write { .. } if self.terminal_busy => BusResponse::Wait,
                BusRequest::Write { value, .. } => {
                    self.accept_terminal_write(address, sanitize_write(value));
                    BusResponse::Written
                }
            };
        }
        if external && self.terminate_unmapped {
            return BusResponse::Wait;
        }
        let (index, writable) = self.map_addr(address);
        match request {
            BusRequest::Read { .. } => BusResponse::Read(sanitize_read(self.ram[index])),
            BusRequest::Write { value, .. } => {
                if writable {
                    self.ram[index] = sanitize_write(value);
                }
                BusResponse::Written
            }
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
                return 0x1000_0000 | u32::from(self.keyboard.get_key());
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
                self.terminal.char0_left = val;
            }
            0x9FC1 => {
                self.terminal.char0_right = val;
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
