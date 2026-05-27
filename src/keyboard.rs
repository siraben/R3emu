use std::collections::VecDeque;

pub struct Keyboard {
    keycode: u8,
    queued: VecDeque<u8>,
}

impl Keyboard {
    pub fn new() -> Self {
        Keyboard {
            keycode: 0,
            queued: VecDeque::new(),
        }
    }

    /// Destructive read: returns the current keycode and resets it to 0.
    pub fn get_key(&mut self) -> u8 {
        if self.keycode != 0 {
            let k = self.keycode;
            self.keycode = 0;
            k
        } else {
            self.queued.pop_front().unwrap_or(0)
        }
    }

    pub fn register_keypress(&mut self, key: u8) {
        self.keycode = key;
    }

    pub fn queue_keypresses<I>(&mut self, keys: I)
    where
        I: IntoIterator<Item = u8>,
    {
        self.queued.extend(keys);
    }
}

#[cfg(test)]
mod tests {
    use super::Keyboard;

    #[test]
    fn queued_input_is_destructive() {
        let mut keyboard = Keyboard::new();
        keyboard.queue_keypresses([b'a', b'b']);

        assert_eq!(keyboard.get_key(), b'a');
        assert_eq!(keyboard.get_key(), b'b');
        assert_eq!(keyboard.get_key(), 0);
    }

    #[test]
    fn live_keypress_takes_priority_over_queued_input() {
        let mut keyboard = Keyboard::new();
        keyboard.queue_keypresses([b'a']);
        keyboard.register_keypress(b'z');

        assert_eq!(keyboard.get_key(), b'z');
        assert_eq!(keyboard.get_key(), b'a');
        assert_eq!(keyboard.get_key(), 0);
    }
}
