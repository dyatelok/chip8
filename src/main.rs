use pixels::{Pixels, SurfaceTexture};
use std::thread;
use std::time::Instant;
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::WindowBuilder,
};

const WIDTH: usize = 64;
const HEIGHT: usize = 32;
const OFFSET: usize = 0x200;
const TARGET_FPS: u64 = 60;
const IPF: usize = 1000; // instructions per frame
const AMIGA_BEHAVIOUR: bool = false;
const MODERN_STR_LD_BEHAVIOUR: bool = false;
const MODERN_SHIFT_BEHAVIOUR: bool = false;
const VF_RESET: bool = true;

#[derive(Clone, Copy)]
struct KeyState {
    pressed_frames_ago: u8,
    released_frames_ago: u8,
}

impl KeyState {
    fn new() -> Self {
        Self {
            pressed_frames_ago: 60,
            released_frames_ago: 60,
        }
    }
    fn press(&mut self) {
        self.pressed_frames_ago = 0;
    }
    fn release(&mut self) {
        self.released_frames_ago = 0;
    }
    fn update_pressed(&mut self) {
        self.pressed_frames_ago += 1;
        self.pressed_frames_ago = self.pressed_frames_ago.min(60);
    }
    fn update_released(&mut self) {
        self.released_frames_ago += 1;
        self.released_frames_ago = self.released_frames_ago.min(60);
    }
    fn is_pressed(&self) -> bool {
        self.pressed_frames_ago <= 3
    }
    fn is_released(&self) -> bool {
        self.released_frames_ago <= 3
    }
}

#[derive(Clone, Copy, Debug)]
enum KeypadKey {
    Key0 = 0x0,
    Key1 = 0x1,
    Key2 = 0x2,
    Key3 = 0x3,
    Key4 = 0x4,
    Key5 = 0x5,
    Key6 = 0x6,
    Key7 = 0x7,
    Key8 = 0x8,
    Key9 = 0x9,
    KeyA = 0xA,
    KeyB = 0xB,
    KeyC = 0xC,
    KeyD = 0xD,
    KeyE = 0xE,
    KeyF = 0xF,
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let window = {
        let size = LogicalSize::new(WIDTH as f64, HEIGHT as f64);
        WindowBuilder::new()
            .with_title("CHIP8")
            .with_inner_size(size)
            .with_min_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };

    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        Pixels::new(WIDTH as u32, HEIGHT as u32, surface_texture).unwrap()
    };

    let mut interpreter = Interpreter::new();

    // interpreter.load("roms/test_opcode.ch8").unwrap();
    // interpreter.load("roms/bc_test.ch8").unwrap();
    // interpreter.load("roms/IBM Logo.ch8").unwrap();
    // interpreter.load("roms/pong.ch8").unwrap();
    // interpreter.load("roms/1-chip8-logo.ch8").unwrap();
    // interpreter.load("roms/2-ibm-logo.ch8").unwrap();
    // interpreter.load("roms/3-corax+.ch8").unwrap();
    // interpreter.load("roms/4-flags.ch8").unwrap();
    // interpreter.load("roms/5-quirks.ch8").unwrap();
    interpreter.load("roms/6-keypad.ch8").unwrap();

    let mut keys = Vec::new();

    let _ = event_loop.run(move |event, elwt| {
        let start_time = Instant::now();
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                println!("The close button was pressed; stopping");
                elwt.exit();
            }
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                logical_key, state, ..
                            },
                        ..
                    },
                ..
            } => {
                keys.push((logical_key, state));
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                if let Err(err) = pixels.resize_surface(size.width, size.height) {
                    eprintln!("pixels.resize_surface error: {err}");
                    elwt.exit();
                }
            }
            Event::AboutToWait => {
                // Application update code.

                // Close events
                if keys.iter().any(|(key, _)| {
                    if let Key::Named(named_key) = key {
                        *named_key == NamedKey::Escape
                    } else {
                        false
                    }
                }) {
                    elwt.exit();
                }

                interpreter.update(&keys[..]);

                // Wait for frame
                let elapsed_time = Instant::now().duration_since(start_time).as_secs_f32();
                let elapsed_millis = (elapsed_time * 1000.0) as u64;
                let wait_millis = if 1000 / TARGET_FPS >= elapsed_millis {
                    1000 / TARGET_FPS - elapsed_millis
                } else {
                    0
                };
                thread::sleep(std::time::Duration::from_millis(wait_millis));

                keys = Vec::new();

                // Redraw the application.
                interpreter.draw(pixels.frame_mut());
                if let Err(err) = pixels.render() {
                    eprintln!("pixels.render error: {err}");
                    elwt.exit();
                }
            }
            _ => (),
        }
    });
}

fn get_key(key: &str) -> Option<KeypadKey> {
    // ╔═══╦═══╦═══╦═══╗       ╔═══╦═══╦═══╦═══╗
    // ║ 1 ║ 2 ║ 3 ║ 4 ║       ║ 1 ║ 2 ║ 3 ║ C ║
    // ╠═══╬═══╬═══╬═══╣       ╠═══╬═══╬═══╬═══╣
    // ║ Q ║ W ║ E ║ R ║       ║ 4 ║ 5 ║ 6 ║ D ║
    // ╠═══╬═══╬═══╬═══╣  -->  ╠═══╬═══╬═══╬═══╣
    // ║ A ║ S ║ D ║ F ║       ║ 7 ║ 8 ║ 9 ║ E ║
    // ╠═══╬═══╬═══╬═══╣       ╠═══╬═══╬═══╬═══╣
    // ║ Z ║ X ║ C ║ V ║       ║ A ║ 0 ║ B ║ F ║
    // ╚═══╩═══╩═══╩═══╝       ╚═══╩═══╩═══╩═══╝
    match key {
        "1" => Some(KeypadKey::Key1),
        "2" => Some(KeypadKey::Key2),
        "3" => Some(KeypadKey::Key3),
        "4" => Some(KeypadKey::KeyC),

        "q" /*| "Q"*/ => Some(KeypadKey::Key4),
        "w" /*| "W"*/ => Some(KeypadKey::Key5),
        "e" /*| "E"*/ => Some(KeypadKey::Key6),
        "r" /*| "R"*/ => Some(KeypadKey::KeyD),

        "a" /*| "A"*/ => Some(KeypadKey::Key7),
        "s" /*| "S"*/ => Some(KeypadKey::Key8),
        "d" /*| "D"*/ => Some(KeypadKey::Key9),
        "f" /*| "F"*/ => Some(KeypadKey::KeyE),

        "z" /*| "Z"*/ => Some(KeypadKey::KeyA),
        "x" /*| "X"*/ => Some(KeypadKey::Key0),
        "c" /*| "C"*/ => Some(KeypadKey::KeyB),
        "v" /*| "V"*/ => Some(KeypadKey::KeyF),

        _ => None,
    }
}

struct Interpreter {
    memory: Vec<u8>,
    screen: [[bool; WIDTH]; HEIGHT],
    program_counter: usize,
    index: u16,
    stack: Vec<u16>,
    delay_timer: u8,
    sound_timer: u8,
    registers: [u8; 16],
    halt: bool,
    keys: [KeyState; 16],
    last_pressed_frames_ago: Option<(KeypadKey, u8)>,
}

impl Interpreter {
    fn new() -> Self {
        Self {
            memory: vec![0; 4096],
            screen: [[false; WIDTH]; HEIGHT],
            program_counter: OFFSET,
            index: 0,
            stack: vec![],
            delay_timer: 0,
            sound_timer: 0,
            registers: [0; 16],
            halt: false,
            keys: [KeyState::new(); 16],
            last_pressed_frames_ago: None,
        }
    }

    fn load(&mut self, filename: &str) -> Result<(), std::io::Error> {
        let bytes = std::fs::read(filename)?;

        // load font
        self.memory[0..80].copy_from_slice(&[
            0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
            0x20, 0x60, 0x20, 0x20, 0x70, // 1
            0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
            0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
            0x90, 0x90, 0xF0, 0x10, 0x10, // 4
            0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
            0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
            0xF0, 0x10, 0x20, 0x40, 0x40, // 7
            0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
            0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
            0xF0, 0x90, 0xF0, 0x90, 0x90, // A
            0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
            0xF0, 0x80, 0x80, 0x80, 0xF0, // C
            0xE0, 0x90, 0x90, 0x90, 0xE0, // D
            0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
            0xF0, 0x80, 0xF0, 0x80, 0x80, // F
        ]);

        // load program
        self.memory[OFFSET..OFFSET + bytes.len()].copy_from_slice(&bytes);
        Ok(())
    }

    fn read_opcode(&self) -> u16 {
        let p = self.program_counter;
        let op_byte1 = self.memory[p] as u16;
        let op_byte2 = self.memory[p + 1] as u16;

        op_byte1 << 8 | op_byte2
    }

    fn exe(&mut self) {
        if self.halt {
            return;
        }

        let opcode = self.read_opcode();
        self.program_counter += 2;

        // println!("{:04x}", opcode);
        // println!("{:?}", self.stack);

        let c = ((opcode & 0xF000) >> 12) as u8;
        let x = ((opcode & 0x0F00) >> 8) as u8;
        let y = ((opcode & 0x00F0) >> 4) as u8;
        let n = (opcode & 0x000F) as u8;

        let nn = opcode & 0x00FF;
        let nnn = opcode & 0x0FFF;

        match (c, x, y, n) {
            (0x0, 0x0, 0xE, 0x0) => {
                //Clear the screen
                for row in &mut self.screen {
                    for pix in row {
                        *pix = false;
                    }
                }
            }
            (0x0, 0x0, 0xE, 0xE) => {
                //Return from a subroutine
                let addr = self.stack.pop().unwrap();
                self.program_counter = addr as usize;
            }
            (0x0, ..) => {
                //TODO Execute machine language subroutine at address NNN
                panic!()
            }
            (0x1, ..) => {
                //Jump to address NNN
                self.program_counter = nnn as usize;
            }
            (0x2, ..) => {
                // Execute subroutine starting at address NNN
                self.stack.push(self.program_counter as u16);
                self.program_counter = nnn as usize;
            }
            (0x3, ..) => {
                //Skip the following instruction if the value of register VX equals NN
                if self.registers[x as usize] == nn as u8 {
                    self.program_counter += 2;
                }
            }
            (0x4, ..) => {
                //Skip the following instruction if the value of register VX is not equal to NN
                if self.registers[x as usize] != nn as u8 {
                    self.program_counter += 2;
                }
            }
            (0x5, ..) => {
                //Skip the following instruction if the value of register VX is equal to the value of register VY
                if self.registers[x as usize] == self.registers[y as usize] {
                    self.program_counter += 2;
                }
            }
            (0x6, ..) => {
                //Store number NN in register VX
                self.registers[x as usize] = nn as u8;
            }
            (0x7, ..) => {
                //Add the value NN to register VX
                self.registers[x as usize] += nn as u8;
            }
            (0x8, _, _, 0x0) => {
                //Store the value of register VY in register VX
                self.registers[x as usize] = self.registers[y as usize];
            }
            (0x8, _, _, 0x1) => {
                //Set VX to VX OR VY
                self.registers[x as usize] |= self.registers[y as usize];
                if VF_RESET {
                    self.registers[0xF] = 0x00;
                }
            }
            (0x8, _, _, 0x2) => {
                //Set VX to VX AND VY
                self.registers[x as usize] &= self.registers[y as usize];
                if VF_RESET {
                    self.registers[0xF] = 0x00;
                }
            }
            (0x8, _, _, 0x3) => {
                //Set VX to VX XOR VY
                self.registers[x as usize] ^= self.registers[y as usize];
                if VF_RESET {
                    self.registers[0xF] = 0x00;
                }
            }
            (0x8, _, _, 0x4) => {
                // Add the value of register VY to register VX
                // Set VF to 01 if a carry occurs
                // Set VF to 00 if a carry does not occur
                let (val, carry) =
                    self.registers[x as usize].overflowing_add(self.registers[y as usize]);

                self.registers[x as usize] = val;
                self.registers[0xF] = if carry { 0x01 } else { 0x00 };
            }
            (0x8, _, _, 0x5) => {
                // Subtract the value of register VY from register VX
                // Set VF to 00 if a borrow occurs
                // Set VF to 01 if a borrow does not occur
                let (val, borrow) =
                    self.registers[x as usize].overflowing_sub(self.registers[y as usize]);

                self.registers[x as usize] = val;
                self.registers[0xF] = if borrow { 0x00 } else { 0x01 };
            }
            (0x8, _, _, 0x6) => {
                // Store the value of register VY shifted right one bit in register VX¹
                // Set register VF to the least significant bit prior to the shift
                // VY is unchanged
                if MODERN_SHIFT_BEHAVIOUR {
                    let bit = self.registers[x as usize] & 0b0000_0001;
                    self.registers[x as usize] >>= 1;
                    self.registers[0xF] = bit;
                } else {
                    let bit = self.registers[y as usize] & 0b0000_0001;
                    self.registers[x as usize] = self.registers[y as usize] >> 1;
                    self.registers[0xF] = bit;
                }
            }
            (0x8, _, _, 0x7) => {
                // Set register VX to the value of VY minus VX
                // Set VF to 00 if a borrow occurs
                // Set VF to 01 if a borrow does not occur
                let (val, borrow) =
                    self.registers[y as usize].overflowing_sub(self.registers[x as usize]);

                self.registers[x as usize] = val;
                self.registers[0xF] = if borrow { 0x00 } else { 0x01 };
            }
            (0x8, _, _, 0xE) => {
                // Store the value of register VY shifted left one bit in register VX¹
                // Set register VF to the most significant bit prior to the shift
                // VY is unchanged
                if MODERN_SHIFT_BEHAVIOUR {
                    let bit = (self.registers[x as usize] & 0b1000_0000) >> 7;
                    self.registers[x as usize] <<= 1;
                    self.registers[0xF] = bit;
                } else {
                    let bit = (self.registers[y as usize] & 0b1000_0000) >> 7;
                    self.registers[x as usize] = self.registers[y as usize] << 1;
                    self.registers[0xF] = bit;
                }
            }
            (0x9, ..) => {
                //Skip the following instruction if the value of register VX is not equal to the value of register VY
                if self.registers[x as usize] != self.registers[y as usize] {
                    self.program_counter += 2;
                }
            }
            (0xA, ..) => {
                self.index = nnn;
            }
            (0xB, ..) => {
                //Jump to address NNN + V0
                self.program_counter = nnn as usize + self.registers[0] as usize;
            }
            (0xC, ..) => {
                //Set VX to a random number with a mask of NN
                self.registers[x as usize] = rand::random::<u8>() & nn as u8;
            }
            (0xD, ..) => {
                // Draw a sprite at position VX, VY with N bytes of sprite data starting at the address stored in I
                // Set VF to 01 if any set pixels are changed to unset, and 00 otherwise
                self.registers[0xF] = 0x00;

                let x = self.registers[x as usize] as usize % WIDTH;
                let y = self.registers[y as usize] as usize % HEIGHT;

                let clipped_n = (y + n as usize).min(HEIGHT) - y;

                for i in 0..clipped_n {
                    let byte = self.memory[self.index as usize + i];

                    let bits: &mut [bool] = &mut self.screen[y + i][x..(x + 8).min(WIDTH)];
                    for (i, bit) in bits.iter_mut().enumerate() {
                        let new = (byte >> (7 - i)) % 2 == 1;
                        if *bit && new {
                            self.registers[0xF] = 0x01;
                        }
                        *bit ^= new;
                    }
                }
            }
            (0xE, _, 0x9, 0xE) => {
                //Skip the following instruction if the key corresponding to the hex value currently stored in register VX is pressed
                if self.is_key_pressed(self.registers[x as usize]) {
                    self.program_counter += 2;
                }
            }
            (0xE, _, 0xA, 0x1) => {
                //Skip the following instruction if the key corresponding to the hex value currently stored in register VX is not pressed
                if !self.is_key_pressed(self.registers[x as usize]) {
                    self.program_counter += 2;
                }
            }
            (0xF, _, 0x0, 0x7) => {
                //Store the current value of the delay timer in register VX
                self.registers[x as usize] = self.delay_timer;
            }
            (0xF, _, 0x0, 0xA) => {
                //TODO	Wait for a keypress and store the result in register VX
                if let Some((key, frames_ago)) = self.last_pressed_frames_ago {
                    if frames_ago == 0 {
                        self.registers[x as usize] = key as u8;
                    } else {
                        self.program_counter -= 2;
                    }
                    //TODO On the original COSMAC VIP, the key was only registered when it was pressed and then released.
                } else {
                    self.program_counter -= 2;
                }
            }
            (0xF, _, 0x1, 0x5) => {
                //Set the delay timer to the value of register VX
                self.delay_timer = self.registers[x as usize];
            }
            (0xF, _, 0x1, 0x8) => {
                //Set the sound timer to the value of register VX
                self.sound_timer = self.registers[x as usize];
            }
            (0xF, _, 0x1, 0xE) => {
                //Add the value stored in register VX to register I
                if AMIGA_BEHAVIOUR {
                    let prev = self.index <= 0xFFF;
                    self.index += self.registers[x as usize] as u16;
                    if prev && self.index > 0x0FFF {
                        self.registers[0xF] = 0x1;
                    } else {
                        self.registers[0xF] = 0x0;
                    }
                } else {
                    self.index += self.registers[x as usize] as u16;
                }
            }
            (0xF, _, 0x2, 0x9) => {
                //Set I to the memory address of the sprite data corresponding to the hexadecimal digit stored in register VX
                self.index = self.registers[x as usize] as u16 * 5; // hardcoded in the load method
            }
            (0xF, _, 0x3, 0x3) => {
                //Store the binary-coded decimal equivalent of the value stored in register VX at addresses I, I + 1, and I + 2
                self.memory[self.index as usize] = self.registers[x as usize] / 100;
                self.memory[self.index as usize + 1] = (self.registers[x as usize] / 10) % 10;
                self.memory[self.index as usize + 2] = self.registers[x as usize] % 10;
            }
            (0xF, _, 0x5, 0x5) => {
                //Store the values of registers V0 to VX inclusive in memory starting at address I
                //I is set to I + X + 1 after operation²
                if MODERN_STR_LD_BEHAVIOUR {
                    for i in 0..=x as usize {
                        self.memory[self.index as usize + i] = self.registers[i];
                    }
                } else {
                    for i in 0..=x as usize {
                        self.memory[self.index as usize] = self.registers[i];
                        self.index += 1;
                    }
                }
            }
            (0xF, _, 0x6, 0x5) => {
                //Fill registers V0 to VX inclusive with the values stored in memory starting at address I
                //I is set to I + X + 1 after operation²
                if MODERN_STR_LD_BEHAVIOUR {
                    for i in 0..=x as usize {
                        self.registers[i] = self.memory[self.index as usize + i];
                    }
                } else {
                    for i in 0..=x as usize {
                        self.registers[i] = self.memory[self.index as usize];
                        self.index += 1;
                    }
                }
            }
            _ => {
                panic!("wrong opcode {:04x}", opcode);
            }
        }
    }

    fn draw(&self, frame: &mut [u8]) {
        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            let rgba = if self.screen[i / WIDTH][i % WIDTH] {
                [0x0, 0x0, 0x0, 0xff]
            } else {
                [0xff, 0xff, 0xff, 0xff]
            };

            pixel.copy_from_slice(&rgba);
        }
    }

    fn update(&mut self, keys: &[(Key, ElementState)]) {
        for key in &mut self.keys {
            key.update_pressed();
            key.update_released();
        }

        let mut last = None;

        for (key, state) in keys {
            if let Some(key) = key.to_text().and_then(get_key) {
                if *state == ElementState::Pressed {
                    last = Some(key);
                }

                match state {
                    ElementState::Pressed => {
                        self.keys[key as usize].press();
                    }
                    ElementState::Released => {
                        self.keys[key as usize].release();
                    }
                }
                // println!("{key:?} {state:?}");
            }
        }

        if let Some(keypad_key) = last {
            self.last_pressed_frames_ago = Some((keypad_key, 0));
        }

        for _ in 0..IPF {
            self.exe();
        }

        self.delay_timer = (self.delay_timer + 59) % 60;
        self.sound_timer = (self.sound_timer + 59) % 60;
        if let Some((_, frames_ago)) = &mut self.last_pressed_frames_ago {
            *frames_ago = (*frames_ago + 1).min(60);
        }
    }

    fn is_key_pressed(&self, key: u8) -> bool {
        self.keys[key as usize].is_pressed()
    }

    fn is_key_released(&self, key: u8) -> bool {
        self.keys[key as usize].is_released()
    }
}
