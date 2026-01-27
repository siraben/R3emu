pub struct Keyboard {
    keycode: u8,
}

impl Keyboard {
    pub fn new() -> Self {
        Keyboard { keycode: 0 }
    }

    /// Destructive read: returns the current keycode and resets it to 0.
    pub fn get_key(&mut self) -> u8 {
        let k = self.keycode;
        self.keycode = 0;
        k
    }

    pub fn register_keypress(&mut self, key: u8) {
        self.keycode = key;
    }
}
