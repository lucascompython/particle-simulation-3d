use std::collections::HashSet;
use winit::{
    event::{ElementState, MouseButton},
    keyboard::KeyCode,
};

pub struct InputManager {
    keys_pressed: HashSet<KeyCode>,
    mouse_buttons_pressed: HashSet<MouseButton>,
    mouse_position: (f32, f32),
    mouse_delta: (f32, f32),
    is_mouse_captured: bool,
    mouse_wheel_delta: f32,
}

impl InputManager {
    pub fn new() -> Self {
        Self {
            keys_pressed: HashSet::new(),
            mouse_buttons_pressed: HashSet::new(),
            mouse_position: (0.0, 0.0),
            mouse_delta: (0.0, 0.0),
            is_mouse_captured: false,
            mouse_wheel_delta: 0.0,
        }
    }

    pub fn handle_keyboard_input(&mut self, input: winit::event::KeyEvent) {
        match input.physical_key {
            winit::keyboard::PhysicalKey::Code(keycode) => match input.state {
                ElementState::Pressed => {
                    self.keys_pressed.insert(keycode);
                }
                ElementState::Released => {
                    self.keys_pressed.remove(&keycode);
                }
            },
            _ => {}
        }
    }

    pub fn handle_mouse_button(&mut self, button: MouseButton, state: ElementState) {
        match state {
            ElementState::Pressed => {
                self.mouse_buttons_pressed.insert(button);
            }
            ElementState::Released => {
                self.mouse_buttons_pressed.remove(&button);
            }
        }
    }

    pub fn handle_mouse_motion(&mut self, x: f32, y: f32) {
        self.mouse_delta = (x - self.mouse_position.0, y - self.mouse_position.1);
        self.mouse_position = (x, y);
    }

    pub fn handle_mouse_wheel(&mut self, delta: f32) {
        self.mouse_wheel_delta = delta;
    }

    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.keys_pressed.contains(&key)
    }

    pub fn is_mouse_button_pressed(&self, button: MouseButton) -> bool {
        self.mouse_buttons_pressed.contains(&button)
    }

    pub fn mouse_position(&self) -> (f32, f32) {
        self.mouse_position
    }

    pub fn mouse_delta(&self) -> (f32, f32) {
        self.mouse_delta
    }

    pub fn set_mouse_captured(&mut self, captured: bool) {
        self.is_mouse_captured = captured;
    }

    pub fn is_mouse_captured(&self) -> bool {
        self.is_mouse_captured
    }

    pub fn reset_mouse_delta(&mut self) {
        self.mouse_delta = (0.0, 0.0);
    }
}
