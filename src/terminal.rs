use std::io::{self, Write};

use crate::font::FONT_8X8;

/// 16-color palette (CGA-style ordering).
pub const COLOR_TABLE: [[u8; 3]; 16] = [
    [0x00, 0x00, 0x00], // 0  black
    [0x00, 0x00, 0xAA], // 1  dark blue
    [0x00, 0xAA, 0x00], // 2  dark green
    [0x00, 0xAA, 0xAA], // 3  dark cyan
    [0xAA, 0x00, 0x00], // 4  dark red
    [0xAA, 0x00, 0xAA], // 5  dark magenta
    [0xAA, 0xAA, 0x00], // 6  dark yellow
    [0xAA, 0xAA, 0xAA], // 7  light grey
    [0x55, 0x55, 0x55], // 8  dark grey
    [0x55, 0x55, 0xFF], // 9  light blue
    [0x55, 0xFF, 0x55], // 10 light green
    [0x55, 0xFF, 0xFF], // 11 light cyan
    [0xFF, 0x55, 0x55], // 12 light red
    [0xFF, 0x55, 0xFF], // 13 light magenta
    [0xFF, 0xFF, 0x55], // 14 light yellow
    [0xFF, 0xFF, 0xFF], // 15 white
];

/// Packed ranges preserve endpoint order, unlike Rust's ascending ranges.
#[derive(Clone, Copy)]
pub(crate) struct DirectedRange {
    first: u8,
    last: u8,
}

impl DirectedRange {
    fn new(value: u16) -> Self {
        Self {
            first: (value & 31) as u8,
            last: ((value >> 5) & 31) as u8,
        }
    }

    fn advance(self, position: u8) -> u8 {
        // The physical mask generator saturates indices at its 29th bit.
        if self.first.min(28) > self.last.min(28) {
            position.wrapping_sub(1) & 31
        } else {
            position.wrapping_add(1) & 31
        }
    }

    fn mask(self) -> u32 {
        let low = self.first.min(self.last).min(28);
        let high = self.first.max(self.last).min(28);
        ((1 << (high + 1)) - 1) & !((1 << low) - 1)
    }
}

fn coordinates(row_oriented: bool, primary: u8, secondary: u8) -> (u8, u8) {
    if row_oriented {
        (primary, secondary)
    } else {
        (secondary, primary)
    }
}

/// Immutable work emitted by the controller and applied by the display later.
pub(crate) enum RenderOperation {
    Scroll {
        row_oriented: bool,
        secondary: DirectedRange,
        mask: u32,
        glyph: [u8; 8],
        colors: u8,
    },
    Pixel {
        column: u8,
        row: u8,
        color: u8,
    },
}

pub struct Terminal {
    pub charsnh: u8,
    pub charsnv: u8,
    /// Pixel buffer: one byte per pixel (color index), row-major.
    /// Size = (8 * charsnh) * (8 * charsnv).
    pub pixbuf: Vec<u8>,
    pub colors: u8,
    pub hrange: u16,
    pub vrange: u16,
    pub cursor: u16,
    pub nlchar: u8,
    pub scrollmask: u32,
    pub char0_right: u32,
    pub char0_left: u32,
    pub mirror_stdout: bool,
}

impl Terminal {
    pub fn new(charsnh: u8, charsnv: u8) -> Self {
        let pix_count = (8 * charsnh as usize) * (8 * charsnv as usize);
        Terminal {
            charsnh,
            charsnv,
            pixbuf: vec![0; pix_count],
            colors: 0,
            hrange: 0,
            vrange: 0,
            cursor: 0,
            nlchar: 0,
            scrollmask: 0,
            char0_right: 0xDEAD_BEEF,
            char0_left: 0xDEAD_BEEF,
            mirror_stdout: false,
        }
    }

    pub fn pixel_width(&self) -> u32 {
        8 * self.charsnh as u32
    }

    pub fn pixel_height(&self) -> u32 {
        8 * self.charsnv as u32
    }

    fn set_pix(&mut self, x: u32, y: u32, color_index: u8) {
        let stride = 8 * self.charsnh as u32;
        let idx = x + y * stride;
        if x < self.pixel_width() && y < self.pixel_height() {
            self.pixbuf[idx as usize] = color_index;
        }
    }

    fn get_pix(&self, x: u32, y: u32) -> u8 {
        let stride = 8 * self.charsnh as u32;
        let idx = x + y * stride;
        if x < self.pixel_width() && y < self.pixel_height() {
            self.pixbuf[idx as usize]
        } else {
            0
        }
    }

    pub fn set_char(&mut self, fcolor: u8, bcolor: u8, index: u8, column: u8, row: u8) {
        self.paint_glyph(fcolor, bcolor, self.glyph(index), column, row);
    }

    fn glyph(&self, index: u8) -> [u8; 8] {
        if index != 0 {
            return FONT_8X8[index as usize];
        }
        // R316 v2 packs left/right halves in column order 0,2,1,3, bottom-to-top,
        // with inverted polarity. The manual's even/odd names are inaccurate;
        // all 64 register-bit positions are verified against the native circuit.
        std::array::from_fn(|y| {
            let mut row = 0;
            for x in 0..8 {
                let columns = if x < 4 {
                    self.char0_left
                } else {
                    self.char0_right
                };
                let byte = (x % 4) / 2 + (x % 2) * 2;
                if columns & (1 << (byte * 8 + 7 - y)) == 0 {
                    row |= 1 << x;
                }
            }
            row
        })
    }

    fn paint_glyph(&mut self, foreground: u8, background: u8, glyph: [u8; 8], column: u8, row: u8) {
        for (y, bits) in glyph.into_iter().enumerate() {
            for x in 0..8 {
                let color = if bits & (1 << x) != 0 {
                    foreground
                } else {
                    background
                };
                self.set_pix(
                    x + u32::from(column) * 8,
                    y as u32 + u32::from(row) * 8,
                    color & 15,
                );
            }
        }
    }

    pub(crate) fn render(&mut self, operation: RenderOperation) {
        match operation {
            RenderOperation::Scroll {
                row_oriented,
                secondary,
                mask,
                glyph,
                colors,
            } => {
                self.scroll_region(row_oriented, secondary, mask, glyph, colors);
            }
            RenderOperation::Pixel { column, row, color } => {
                self.set_pix(u32::from(column), u32::from(row), color);
            }
        }
    }

    fn copy_char_pix(&mut self, sx: u8, sy: u8, dx: u8, dy: u8) {
        let sx = sx as u32 * 8;
        let sy = sy as u32 * 8;
        let dx = dx as u32 * 8;
        let dy = dy as u32 * 8;
        for y in 0u32..8 {
            for x in 0u32..8 {
                let c = self.get_pix(sx + x, sy + y);
                self.set_pix(dx + x, dy + y, c);
            }
        }
    }

    /// Plot a single pixel via the pixel-plot I/O range.
    /// `offset` = address offset from 0x9FE0 (lower 4 bits = color index).
    pub fn plot_pixel(&mut self, val: u32, offset: u16) {
        let color_index = (offset & 0b1111) as u8;
        let row = (val >> 8) & 0xFF;
        let column = val & 0xFF;
        self.set_pix(column, row, color_index);
    }

    /// Render a complete scrollprint synchronously.
    ///
    /// Automatic scrolling has two rendering steps. The frame-driven bus must
    /// schedule those separately; this convenience method completes both.
    pub fn scroll_print(&mut self, value: u32, offset: u16) {
        let (operation, retry) = self.plan_scroll_print(value, offset);
        if let Some(operation) = operation {
            self.render(operation);
        }
        if retry {
            let (operation, _) = self.plan_scroll_print(value, offset);
            if let Some(operation) = operation {
                self.render(operation);
            }
        }
    }

    /// Advance the controller without changing visible pixels. A true retry
    /// requests the second frame of an automatic scroll.
    pub(crate) fn plan_scroll_print(
        &mut self,
        value: u32,
        offset: u16,
    ) -> (Option<RenderOperation>, bool) {
        let terminal_mode = offset & 1 != 0;
        let row_oriented = offset & 4 != 0;
        let masked = offset & 8 != 0;
        let auto_scroll = offset & 16 != 0;
        let character = value as u8;
        let newline = offset & 32 != 0 && character == self.nlchar;
        let colors = if offset & 2 != 0 {
            (value >> 8) as u8
        } else {
            self.colors
        };
        let (primary, secondary) = if row_oriented {
            (
                DirectedRange::new(self.hrange),
                DirectedRange::new(self.vrange),
            )
        } else {
            (
                DirectedRange::new(self.vrange),
                DirectedRange::new(self.hrange),
            )
        };
        let column = (self.cursor & 31) as u8;
        let row = ((self.cursor >> 5) & 31) as u8;
        let (mut p, mut s) = coordinates(row_oriented, column, row);
        let wrap = p == primary.advance(primary.last) || newline;
        let mut entire_range = !terminal_mode;
        let mut retry = false;
        let mut print = true;
        let mut glyph = character;

        if wrap {
            p = primary.first;
            let old_s = s;
            s = secondary.advance(s);
            if old_s == secondary.last {
                if auto_scroll {
                    s = secondary.last;
                    glyph = self.nlchar;
                    if terminal_mode {
                        entire_range = true;
                        retry = !newline;
                    }
                } else {
                    s = secondary.first;
                    if newline && terminal_mode {
                        print = false;
                    }
                }
            }
        }

        let range = if entire_range {
            secondary
        } else {
            DirectedRange { first: s, last: s }
        };
        let mask = if masked {
            self.scrollmask & 0x1FFF_FFFF
        } else if entire_range {
            primary.mask()
        } else {
            DirectedRange { first: p, last: p }.mask()
        };
        let operation = print.then(|| RenderOperation::Scroll {
            row_oriented,
            secondary: range,
            mask,
            glyph: self.glyph(glyph),
            colors,
        });

        if !newline && !retry {
            p = primary.advance(p);
        }
        if terminal_mode {
            let (column, row) = coordinates(row_oriented, p, s);
            self.cursor = u16::from(column) | (u16::from(row) << 5);
        }
        if self.mirror_stdout && terminal_mode && !retry {
            let byte = if newline { b'\n' } else { character };
            let _ = io::stdout().write_all(&[byte]);
        }
        (operation, retry)
    }

    fn scroll_region(
        &mut self,
        row_oriented: bool,
        secondary: DirectedRange,
        mask: u32,
        glyph: [u8; 8],
        colors: u8,
    ) {
        // Copy toward the vacated edge, never overwriting a source before it is
        // read. Swapping both coordinates handles column-oriented scrolling.
        for p in 0..29 {
            if mask & (1 << p) == 0 {
                continue;
            }
            let mut s = secondary.first;
            while s != secondary.last {
                let next = secondary.advance(s);
                let (sx, sy) = coordinates(row_oriented, p, next);
                let (dx, dy) = coordinates(row_oriented, p, s);
                self.copy_char_pix(sx, sy, dx, dy);
                s = next;
            }
            let (column, row) = coordinates(row_oriented, p, secondary.last);
            self.paint_glyph(colors & 15, colors >> 4, glyph, column, row);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn programmable_character_maps_each_bit_to_its_column_and_row() {
        for column in 0..8 {
            for row in 0..8 {
                let mut terminal = Terminal::new(1, 1);
                terminal.char0_left = u32::MAX;
                terminal.char0_right = u32::MAX;
                let byte = [0, 2, 1, 3][column as usize % 4];
                let bit = 1 << (byte * 8 + 7 - row);
                if column < 4 {
                    terminal.char0_left &= !bit;
                } else {
                    terminal.char0_right &= !bit;
                }
                terminal.set_char(15, 3, 0, 0, 0);
                for y in 0..8 {
                    for x in 0..8 {
                        let expected = if (x, y) == (column, row) { 15 } else { 3 };
                        assert_eq!(terminal.get_pix(x, y), expected);
                    }
                }
            }
        }
    }

    #[test]
    fn upper_character_rom_and_control_placeholders_are_not_blank() {
        let mut terminal = Terminal::new(1, 1);
        for index in [1, 9, 128, 160, 255] {
            terminal.set_char(15, 0, index, 0, 0);
            for y in 0..8 {
                for x in 0..8 {
                    let lit = (x / 2 + y / 2) % 2 == 0;
                    assert_eq!(terminal.get_pix(x, y), if lit { 15 } else { 0 });
                }
            }
        }
    }

    #[test]
    fn out_of_bounds_pixels_do_not_wrap_to_next_row() {
        let mut terminal = Terminal::new(1, 1);
        terminal.plot_pixel(8, 15);
        assert!(terminal.pixbuf.iter().all(|&pixel| pixel == 0));
        terminal.set_pix(0, 1, 3);
        assert_eq!(terminal.get_pix(8, 0), 0);
        assert_eq!(terminal.get_pix(0, 1), 3);
    }
}
