use error_iter::ErrorIter as _;
use log::error;
use pixels::{Error, Pixels, SurfaceTexture};
use std::time::Instant;
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

const WIDTH: u32 = 64;
const HEIGHT: u32 = 32;
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
        Pixels::new(WIDTH, HEIGHT, surface_texture)?
    };

    let mut interpreter = Interpreter::new();
    interpreter.load("IBM Logo.ch8").unwrap();

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
    screen: [bool; (WIDTH * HEIGHT) as usize],
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
            screen: [false; (WIDTH * HEIGHT) as usize],
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
        self.memory[0..bytes.len()].copy_from_slice(&bytes);
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
            (0x0, 0x0, 0x0, 0x0) => {
                self.halt = true;
                println!("program halt!");
            }
            (0x0, 0x0, 0xE, 0x0) => {
                for pix in &mut self.screen {
                    *pix = false;
                }
            }
            (0x1, _, _, _) => {
                self.program_counter = nnn as usize;
            }
            (0x6, _, _, _) => {
                self.registers[x as usize] = nn as u8;
            }
            (0x7, _, _, _) => {
                self.registers[x as usize] += nn as u8;
            }
            (0xA, _, _, _) => {
                self.index = nnn;
            }
            (0xD, _, _, _) => {
                for i in 0..n {
                    self.display_byte(x, y + i, self.memory[self.program_counter]);
                    self.program_counter += 1;
                }
            }
            _ => panic!(
                "opcode {:04x} has not yet been implemented or just a wrong opcode",
                opcode
            ),
        }
    }

    fn display_byte(&mut self, x: u8, y: u8, byte: u8) {
        let pos = (x as u32 + y as u32 * WIDTH) as usize;
        let bytes: &mut [bool] = &mut self.screen[pos..pos + 8];
        for (i, bit) in bytes.iter_mut().enumerate() {
            *bit = (byte >> (7 - i)) % 2 == 1;
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
            let rgba = if self.screen[i] {
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
