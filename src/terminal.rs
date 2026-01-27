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
    pub char0even: u32,
    pub char0odd: u32,
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
            nlchar: b'\n',
            scrollmask: 0,
            char0even: 0,
            char0odd: 0,
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
        if (idx as usize) < self.pixbuf.len() {
            self.pixbuf[idx as usize] = color_index;
        }
    }

    fn get_pix(&self, x: u32, y: u32) -> u8 {
        let stride = 8 * self.charsnh as u32;
        let idx = x + y * stride;
        if (idx as usize) < self.pixbuf.len() {
            self.pixbuf[idx as usize]
        } else {
            0
        }
    }

    pub fn set_char(
        &mut self,
        fcolor: u8,
        bcolor: u8,
        charindex: u8,
        column: u8,
        row: u8,
    ) {
        let ci = if charindex > 127 { 0 } else { charindex };
        for y in 0u8..8 {
            for x in 0u8..8 {
                let lit = (FONT_8X8[ci as usize][y as usize] >> x) & 1 != 0;
                let color = if lit { fcolor } else { bcolor };
                self.set_pix(
                    x as u32 + column as u32 * 8,
                    y as u32 + row as u32 * 8,
                    color,
                );
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
        let row = ((val >> 8) & 0xFF) as u32;
        let column = (val & 0xFF) as u32;
        self.set_pix(column, row, color_index);
    }

    /// Scroll-print I/O handler.
    /// `val` = value written, `offset` = address offset from I/O base (0..=0x3F).
    /// The offset bits encode control flags.
    pub fn scroll_print(&mut self, val: u32, offset: u16) {
        let nlchar_flag = (offset >> 5) & 1 != 0;
        let tmscroll = (offset >> 4) & 1 != 0;
        // bit 3 unused
        let roprint = (offset >> 2) & 1 != 0;
        let cfdata = (offset >> 1) & 1 != 0;
        let etmode = offset & 1 != 0;

        let mut column = (self.cursor & 0b11111) as u8;
        let mut row = ((self.cursor >> 5) & 0b11111) as u8;

        let (prange, srange) = if roprint {
            (self.hrange, self.vrange)
        } else {
            (self.vrange, self.hrange)
        };

        let pend = ((prange >> 5) & 0b11111) as u8;
        let pstart = (prange & 0b11111) as u8;
        let send = ((srange >> 5) & 0b11111) as u8;
        let sstart = (srange & 0b11111) as u8;

        let forecolor = if !cfdata {
            self.colors & 0b1111
        } else {
            ((val >> 8) & 0b1111) as u8
        };
        let backcolor = if !cfdata {
            (self.colors >> 4) & 0b1111
        } else {
            ((val >> 13) & 0b1111) as u8
        };
        let charindex = (val & 0xFF) as u8;

        // Map primary/secondary to column/row
        let mut pval = if roprint { column } else { row };
        let mut sval = if roprint { row } else { column };

        // Newline character handling
        if nlchar_flag && charindex == self.nlchar {
            pval = pend.wrapping_add(1);
        }

        if etmode {
            // Character print mode
            if pval > pend {
                pval = 0;
                sval = sval.wrapping_add(1);
            }
            if sval > send {
                if tmscroll {
                    // Scroll: copy lines up
                    for y in sstart..=send {
                        for x in pstart..=pend {
                            self.copy_char_pix(x, y.wrapping_add(1), x, y);
                        }
                    }
                    // Fill last line
                    for x in pstart..=pend {
                        self.set_char(forecolor, backcolor, self.nlchar, x, send);
                    }
                    sval = sval.wrapping_sub(1);
                } else {
                    sval = sstart;
                }
            }

            if !(nlchar_flag && charindex == self.nlchar) {
                // Write character at current position
                let (col, rw) = if roprint {
                    (pval, sval)
                } else {
                    (sval, pval)
                };
                self.set_char(forecolor, backcolor, charindex, col, rw);
                pval = pval.wrapping_add(1);
            }
        } else {
            // Scroll-only mode
            for y in sstart..send {
                for x in pstart..=pend {
                    self.copy_char_pix(x, y.wrapping_add(1), x, y);
                }
            }
            // Fill last line with the character
            for x in pstart..=pend {
                self.set_char(forecolor, backcolor, charindex, x, send);
            }
        }

        // Write back cursor
        if roprint {
            column = pval;
            row = sval;
        } else {
            row = pval;
            column = sval;
        }
        self.cursor = column as u16 + ((row as u16) << 5);
    }
}
