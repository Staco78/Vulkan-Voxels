use std::collections::HashSet;

use winit::event::VirtualKeyCode;

pub struct Inputs {
    keys: HashSet<VirtualKeyCode>,
    pub mouse_delta: (f64, f64),
}

impl Inputs {
    pub fn new() -> Self {
        Self {
            keys: HashSet::new(),
            mouse_delta: (0.0, 0.0),
        }
    }

    #[inline]
    pub fn key_pressed(&mut self, key: VirtualKeyCode) {
        self.keys.insert(key);
    }

    #[inline]
    pub fn key_released(&mut self, key: VirtualKeyCode) {
        self.keys.remove(&key);
    }

    #[inline]
    pub fn mouse_moved(&mut self, delta: (f64, f64)) {
        self.mouse_delta.0 += delta.0;
        self.mouse_delta.1 += delta.1;
    }

    // this should called after rendering
    pub fn reset(&mut self) {
        self.mouse_delta = (0.0, 0.0);
    }

    #[inline]
    pub fn is_key_pressed(&self, key: VirtualKeyCode) -> bool {
        self.keys.contains(&key)
    }
}
