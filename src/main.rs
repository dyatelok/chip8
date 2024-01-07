use error_iter::ErrorIter as _;
use log::error;
use pixels::{Error, Pixels, SurfaceTexture};
use rand::prelude::*;
use std::time::Instant;
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

const WIDTH: usize = 64;
const HEIGHT: usize = 32;
const TARGET_FPS: u64 = 60;
const IPF: usize = 12; // instructions per frame

fn main() -> Result<(), Error> {
    env_logger::init();
    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();
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
        Pixels::new(WIDTH as u32, HEIGHT as u32, surface_texture)?
    };

    let mut interpreter = Interpreter::new();
    interpreter.load("roms/IBM Logo.ch8").unwrap();

    event_loop.run(move |event, _, control_flow| {
        let start_time = Instant::now();

        // Draw the current frame
        if let Event::RedrawRequested(_) = event {
            interpreter.draw(pixels.frame_mut());
            if let Err(err) = pixels.render() {
                log_error("pixels.render", err);
                *control_flow = ControlFlow::Exit;
                return;
            }
        }

        // Handle input events
        if input.update(&event) {
            // Close events
            if input.key_pressed(VirtualKeyCode::Escape) || input.close_requested() {
                *control_flow = ControlFlow::Exit;
                return;
            }

            // Resize the window
            if let Some(size) = input.window_resized() {
                if let Err(err) = pixels.resize_surface(size.width, size.height) {
                    log_error("pixels.resize_surface", err);
                    *control_flow = ControlFlow::Exit;
                    return;
                }
            }

            // Update internal state and request a redraw
            interpreter.update();
            window.request_redraw();

            let elapsed_time = Instant::now().duration_since(start_time).as_secs_f32();
            let elapsed_millis = (elapsed_time * 1000.0) as u64;
            let wait_millis = if 1000 / TARGET_FPS >= elapsed_millis {
                1000 / TARGET_FPS - elapsed_millis
            } else {
                0
            };
            let wait_until = start_time + std::time::Duration::from_millis(wait_millis);
            *control_flow = ControlFlow::WaitUntil(wait_until);
        }
    });
}

fn log_error<E: std::error::Error + 'static>(method_name: &str, err: E) {
    error!("{method_name}() failed: {err}");
    for source in err.sources().skip(1) {
        error!("  Caused by: {source}");
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
    stack_pointer: usize,
    halt: bool,
}

impl Interpreter {
    fn new() -> Self {
        Self {
            memory: vec![0; 4096],
            screen: [[false; WIDTH]; HEIGHT],
            program_counter: 0,
            index: 0,
            stack: vec![0; 16],
            delay_timer: 0,
            sound_timer: 0,
            registers: [0; 16],
            stack_pointer: 0,
            halt: false,
        }
    }

    fn load(&mut self, filename: &str) -> Result<(), std::io::Error> {
        let bytes = std::fs::read(filename)?;
        let offset = 0x200;
        self.memory[offset..offset + bytes.len()].copy_from_slice(&bytes);
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
                todo!()
            }
            (0x0, ..) => {
                //TODO Execute machine language subroutine at address NNN
                // panic!()
            }
            (0x1, ..) => {
                //Jump to address NNN
                self.program_counter = nnn as usize;
            }
            (0x2, ..) => {
                // Execute subroutine starting at address NNN
                self.program_counter = nnn as usize;
                self.stack.push(nnn);
                todo!()
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
            }
            (0x8, _, _, 0x2) => {
                //Set VX to VX AND VY
                self.registers[x as usize] &= self.registers[y as usize];
            }
            (0x8, _, _, 0x3) => {
                //Set VX to VX XOR VY
                self.registers[x as usize] ^= self.registers[y as usize];
            }
            (0x8, _, _, 0x4) => {
                //TODO Add the value of register VY to register VX
                //Set VF to 01 if a carry occurs
                //Set VF to 00 if a carry does not occur
                self.registers[x as usize] += self.registers[y as usize];
            }
            (0x8, _, _, 0x5) => {
                //TODO Subtract the value of register VY from register VX
                //Set VF to 00 if a borrow occurs
                //Set VF to 01 if a borrow does not occur
                self.registers[x as usize] -= self.registers[y as usize];
            }
            (0x8, _, _, 0x6) => {
                //TODO Store the value of register VY shifted right one bit in register VX¹
                // Set register VF to the least significant bit prior to the shift
                // VY is unchanged
                self.registers[x as usize] = self.registers[y as usize] >> 1;
            }
            (0x8, _, _, 0x7) => {
                //TODO Set register VX to the value of VY minus VX
                //Set VF to 00 if a borrow occurs
                //Set VF to 01 if a borrow does not occur
                self.registers[x as usize] =
                    self.registers[y as usize] - self.registers[x as usize];
            }
            (0x8, _, _, 0xE) => {
                //Store the value of register VY shifted left one bit in register VX¹
                //Set register VF to the most significant bit prior to the shift
                //VY is unchanged
                self.registers[x as usize] = self.registers[y as usize] << 1;
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
                //TODO Draw a sprite at position VX, VY with N bytes of sprite data starting at the address stored in I
                // Set VF to 01 if any set pixels are changed to unset, and 00 otherwise
                for i in 0..n as usize {
                    let byte = self.memory[self.index as usize + i];
                    let x = self.registers[x as usize] as usize % WIDTH;
                    let y = self.registers[y as usize] as usize % HEIGHT;

                    let bits: &mut [bool] = &mut self.screen[y + i][x..x + 8];
                    for (i, bit) in bits.iter_mut().enumerate() {
                        *bit = (byte >> (7 - i)) % 2 == 1;
                    }
                }
            }
            (0xE, _, 0x9, 0xE) => {
                //TODO Skip the following instruction if the key corresponding to the hex value currently stored in register VX is pressed
                todo!()
            }
            (0xE, _, 0xA, 0x1) => {
                //TODO Skip the following instruction if the key corresponding to the hex value currently stored in register VX is not pressed
                todo!()
            }
            (0xF, _, 0x0, 0x7) => {
                //TODO	Store the current value of the delay timer in register VX
                todo!()
            }
            (0xF, _, 0x0, 0xA) => {
                //TODO	Wait for a keypress and store the result in register VX
                todo!()
            }
            (0xF, _, 0x1, 0x5) => {
                //TODO	Set the delay timer to the value of register VX
                todo!()
            }
            (0xF, _, 0x1, 0x8) => {
                //TODO	Set the sound timer to the value of register VX
                todo!()
            }
            (0xF, _, 0x1, 0xE) => {
                //TODO	Add the value stored in register VX to register I
                todo!()
            }
            (0xF, _, 0x2, 0x9) => {
                //TODO	Set I to the memory address of the sprite data corresponding to the hexadecimal digit stored in register VX
                todo!()
            }
            (0xF, _, 0x3, 0x3) => {
                //TODO	Store the binary-coded decimal equivalent of the value stored in register VX at addresses I, I + 1, and I + 2
                todo!()
            }
            (0xF, _, 0x5, 0x5) => {
                //TODO	Store the values of registers V0 to VX inclusive in memory starting at address I
                //I is set to I + X + 1 after operation²
                todo!()
            }
            (0xF, _, 0x6, 0x5) => {
                //TODO	Fill registers V0 to VX inclusive with the values stored in memory starting at address I
                //I is set to I + X + 1 after operation²
                todo!()
            }
            _ => {
                panic!(
                    "opcode {:04x} has not yet been implemented or just a wrong opcode",
                    opcode
                );
            }
        }
    }

    // fn call(&mut self, addr: u16) {
    //     let sp = self.stack_pointer;
    //     let stack = &mut self.stack;

    //     if sp > stack.len() {
    //         panic!("Stack overflow!")
    //     }

    //     stack[sp] = self.program_counter as u16;
    //     self.stack_pointer += 1;
    //     self.program_counter = addr as usize;
    // }

    // fn ret(&mut self) {
    //     if self.stack_pointer == 0 {
    //         panic!("Stack underflow");
    //     }

    //     self.stack_pointer -= 1;
    //     let call_addr = self.stack[self.stack_pointer];
    //     self.program_counter = call_addr as usize;
    // }

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

    fn update(&mut self) {
        for _ in 0..IPF {
            self.exe();
        }
    }
}

// 0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
// 0x20, 0x60, 0x20, 0x20, 0x70, // 1
// 0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
// 0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
// 0x90, 0x90, 0xF0, 0x10, 0x10, // 4
// 0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
// 0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
// 0xF0, 0x10, 0x20, 0x40, 0x40, // 7
// 0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
// 0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
// 0xF0, 0x90, 0xF0, 0x90, 0x90, // A
// 0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
// 0xF0, 0x80, 0x80, 0x80, 0xF0, // C
// 0xE0, 0x90, 0x90, 0x90, 0xE0, // D
// 0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
// 0xF0, 0x80, 0xF0, 0x80, 0x80  // F

// fn main() {
//     cpu.registers[0] = 5;
//     cpu.registers[1] = 10;

//     let mem = &mut cpu.memory;
//     mem[0x000] = 0x21;
//     mem[0x001] = 0x00;
//     mem[0x002] = 0x21;
//     mem[0x003] = 0x00;
//     mem[0x004] = 0x00;
//     mem[0x005] = 0x00;

//     mem[0x100] = 0x80;
//     mem[0x101] = 0x14;
//     mem[0x102] = 0x80;
//     mem[0x103] = 0x14;
//     mem[0x104] = 0x00;
//     mem[0x105] = 0xEE;

//     cpu.run();

//     assert_eq!(cpu.registers[0], 45);
//     println!("5 + (10 * 2) + (10 * 2) = {}", cpu.registers[0]);
// }
