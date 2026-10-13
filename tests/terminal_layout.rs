use r3emu::terminal::Terminal;

fn packed(first: u8, last: u8) -> u16 {
    u16::from(first) | (u16::from(last) << 5)
}

#[test]
fn scrolling_matches_an_independent_snapshot_for_every_small_rectangle_and_mask() {
    for horizontal in [false, true] {
        for pfirst in 0..4 {
            for plast in 0..4 {
                for sfirst in 0..4 {
                    for slast in 0..4 {
                        // None selects the range; every four-bit mask replaces it.
                        for mask in std::iter::once(None).chain((0..16).map(Some)) {
                            let mut terminal = Terminal::new(4, 4);
                            for row in 0..4 {
                                for column in 0..4 {
                                    terminal.set_char(0, row * 4 + column, b' ', column, row);
                                }
                            }
                            let (hrange, vrange) = if horizontal {
                                (packed(pfirst, plast), packed(sfirst, slast))
                            } else {
                                (packed(sfirst, slast), packed(pfirst, plast))
                            };
                            terminal.hrange = hrange;
                            terminal.vrange = vrange;
                            terminal.colors = 0xE0;
                            terminal.cursor = packed(2, 3);
                            terminal.scrollmask = mask.unwrap_or(0);
                            let before = terminal.pixbuf.clone();
                            let mut expected = before.clone();
                            // This oracle uses a snapshot and destination mapping,
                            // independently of the implementation's in-place loops.
                            for row in 0..4 {
                                for column in 0..4 {
                                    let (p, s) = if horizontal {
                                        (column, row)
                                    } else {
                                        (row, column)
                                    };
                                    let selected = mask.map_or(
                                        p >= pfirst.min(plast) && p <= pfirst.max(plast),
                                        |bits| bits & (1 << p) != 0,
                                    );
                                    if !selected || s < sfirst.min(slast) || s > sfirst.max(slast) {
                                        continue;
                                    }
                                    for y in 0..8 {
                                        for x in 0..8 {
                                            let dest = (usize::from(row) * 8 + y) * 32
                                                + usize::from(column) * 8
                                                + x;
                                            expected[dest] = if s == slast {
                                                14
                                            } else {
                                                let next =
                                                    if sfirst < slast { s + 1 } else { s - 1 };
                                                let (sx, sy) =
                                                    if horizontal { (p, next) } else { (next, p) };
                                                before[(usize::from(sy) * 8 + y) * 32
                                                    + usize::from(sx) * 8
                                                    + x]
                                            };
                                        }
                                    }
                                }
                            }
                            let offset =
                                if horizontal { 4 } else { 0 } | if mask.is_some() { 8 } else { 0 };
                            terminal.scroll_print(u32::from(b' '), offset);
                            assert_eq!(terminal.pixbuf, expected,
                                "horizontal={horizontal}, p={pfirst}..{plast}, s={sfirst}..{slast}, mask={mask:?}");
                            assert_eq!(terminal.cursor, packed(2, 3), "scroll-only moved cursor");
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn reversed_terminal_ranges_advance_and_wrap_in_both_orientations() {
    for horizontal in [false, true] {
        let mut terminal = Terminal::new(4, 4);
        terminal.colors = 15;
        let coords = |p, s| if horizontal { (p, s) } else { (s, p) };
        let (h, v) = coords(packed(3, 1), packed(2, 0));
        terminal.hrange = h;
        terminal.vrange = v;
        let (column, row) = coords(3, 2);
        terminal.cursor = packed(column as u8, row as u8);
        let offset = if horizontal { 5 } else { 1 };
        for (glyph, p, s, next_p, next_s) in [
            (b'A', 3, 2, 2, 2),
            (b'B', 2, 2, 1, 2),
            (b'C', 1, 2, 0, 2),
            (b'D', 3, 1, 2, 1),
        ] {
            terminal.scroll_print(u32::from(glyph), offset);
            let (column, row) = coords(p, s);
            let mut expected = Terminal::new(4, 4);
            expected.set_char(15, 0, glyph, column as u8, row as u8);
            for y in 0..8 {
                for x in 0..8 {
                    let index = (usize::from(row) * 8 + y) * 32 + usize::from(column) * 8 + x;
                    assert_eq!(terminal.pixbuf[index], expected.pixbuf[index]);
                }
            }
            let (column, row) = coords(next_p, next_s);
            assert_eq!(terminal.cursor, packed(column as u8, row as u8));
        }
    }
}
