use rand;
use super::instruction::{Instruction, Addr, Byte};
use super::font::{FONT_SET};


const OPCODE_SIZE: usize = 2;
const CHIP8_RAM_SIZE: usize = 4096;
pub const CHIP8_WIDTH: usize = 64;
pub const CHIP8_HEIGHT: usize = 32;


enum ProgramCounter {
    Next,
    Skip,
    Jump(usize),
}

impl ProgramCounter {
    pub fn skip_if(cond: bool) -> ProgramCounter {
        if cond {
            ProgramCounter::Skip
        } else {
            ProgramCounter::Next
        }
    }
}

pub struct OutputState<'a> {
    pub vram: &'a [[u8; CHIP8_WIDTH]; CHIP8_HEIGHT],
    pub vram_changed: bool,
    pub beep: bool,
}

pub struct VM {
    ram: [u8; CHIP8_RAM_SIZE],
    vram: [[u8; CHIP8_WIDTH]; CHIP8_HEIGHT],  // graphics memory
    vram_changed: bool,
    stack: [usize; 16],
    v: [u8; 16],  // cpu registers
    i: u16,
    pc: usize,
    sp: usize,
    delay_timer: u8,
    sound_timer: u8,
    keypad: [bool; 16],
    keypad_waiting: bool, // ?
    keypad_register: usize, // ?
}

impl VM {
    pub fn new() -> Self {
        let mut ram = [0; CHIP8_RAM_SIZE];
        for i in 0..FONT_SET.len() {
            ram[i] = FONT_SET[i];
        }

        Self {
            vram: [[0; CHIP8_WIDTH]; CHIP8_HEIGHT],
            vram_changed: false,
            ram: ram,
            v: [0; 16],
            stack: [0; 16],
            i: 0,
            pc: 0x200,
            sp: 0,
            keypad: [false; 16],
            keypad_waiting: false,
            keypad_register: 0,
            delay_timer: 0,
            sound_timer: 0,
        }
    }

    pub fn step(&mut self, keypad: [bool; 16]) -> OutputState {
        self.vram_changed = false;

        for i in 0..keypad.len() {
            self.keypad[i] = keypad[i];
        }

        if self.keypad_waiting {
            for i in 0..keypad.len() {
                if keypad[i] {
                    self.keypad_waiting = false;
                    self.v[self.keypad_register] = i as u8;
                    break;
                }
            }
        } else {
            if self.delay_timer > 0 {
                self.delay_timer -= 1;
            }
            if self.sound_timer > 0 {
                self.sound_timer -= 1;
            }
            let opcode = self.get_opcode();
            self.run_opcode(opcode);
        }

        OutputState {
            vram: &self.vram,
            vram_changed: self.vram_changed,
            beep: self.sound_timer > 0,
        }
    }

    pub fn run_opcode(&mut self, opcode: u16) {
        let pc_change = match Instruction::decode(opcode) {
            Instruction::Clear => self.op_clear(),
            Instruction::Sys(_) => ProgramCounter::Next,
            Instruction::Return => self.op_return(),
            Instruction::Jump(addr) => self.op_jump(addr),
            Instruction::Call(addr) => self.op_call(addr),
            Instruction::SkipEqualK(x, k) => self.op_skip_equal_k(x as usize, k),
            Instruction::SkipNotEqualK(x, k) => self.op_skip_not_equal_k(x as usize, k),
            Instruction::SkipEqual(x, y) => self.op_skip_equal(x as usize, y as usize),
            Instruction::LoadK(x, k) => self.op_load_k(x as usize, k), 
            Instruction::AddK(x, k) => self.op_add_k(x as usize, k),
            Instruction::Set(x, y) => self.op_set(x as usize, y as usize),
            Instruction::Or(x, y) => self.op_or(x as usize, y as usize),
            Instruction::And(x, y) => self.op_and(x as usize, y as usize),
            Instruction::Xor(x, y) => self.op_xor(x as usize, y as usize),
            Instruction::Add(x, y) => self.op_add(x as usize, y as usize),
            Instruction::Sub(x, y) => self.op_sub(x as usize, y as usize),
            Instruction::ShiftRight(x) => self.op_shift_right(x as usize),
            Instruction::SubInv(x, y) => self.op_sub_inv(x as usize, y as usize),
            Instruction::ShiftLeft(x) => self.op_shift_left(x as usize),
            Instruction::SkipNotEqual(x, y) => self.op_skip_not_equal(x as usize, y as usize),
            Instruction::LoadI(addr) => self.op_load_i(addr),
            Instruction::LongJump(addr) => self.op_long_jump(addr),
            Instruction::Rand(x, k) => self.op_rand(x as usize, k),
            Instruction::Draw(x, y, k) => self.op_draw(x as usize, y as usize, k as usize),
            Instruction::SkipPressed(x) => self.op_skip_pressed(x as usize),
            Instruction::SkipNotPressed(x) => self.op_skip_not_pressed(x as usize),
            Instruction::GetTimer(x) => self.op_get_timer(x as usize),
            Instruction::WaitKey(x) => self.op_wait_key(x as usize),
            Instruction::SetTimer(x) => self.op_set_timer(x as usize),
            Instruction::SetSoundTimer(x) => self.op_set_sound_timer(x as usize),
            Instruction::AddI(x) => self.op_add_i(x as usize),
            Instruction::LoadHexGlyph(x) => self.op_load_hex_glyph(x as usize),
            Instruction::StoreBCD(x) => self.op_store_bcd(x as usize),
            Instruction::StoreRegisters(x) => self.op_store_registers(x as usize),
            Instruction::LoadRegisters(x) => self.op_load_registers(x as usize),
            Instruction::Unknown => ProgramCounter::Next,
        };

        match pc_change {
            ProgramCounter::Next => self.pc += OPCODE_SIZE,
            ProgramCounter::Skip => self.pc += OPCODE_SIZE * 2,
            ProgramCounter::Jump(addr) => self.pc = addr
        }
    }

    pub fn load(&mut self, data: &[u8]) {
        for (i, &byte) in data.iter().enumerate() {
            let addr = 0x200 + i;
            if addr >= 4096 {
                break
            }
            self.ram[addr] = byte;
        }
    }

    fn get_opcode(&self) -> u16 {
        (self.ram[self.pc] as u16) << 8 | (self.ram[self.pc+1] as u16)
    }

    fn op_clear(&mut self) -> ProgramCounter {
        for i in 0..CHIP8_HEIGHT {
            for j in 0..CHIP8_WIDTH {
                self.vram[i][j] = 0;
            }
        }
        self.vram_changed = true;
        ProgramCounter::Next
    }

    fn op_return(&mut self) -> ProgramCounter {
        self.sp -= 1;
        ProgramCounter::Jump(self.stack[self.sp])
    }

    fn op_jump(&mut self, addr: Addr) -> ProgramCounter {
        ProgramCounter::Jump(addr as usize)
    }

    fn op_call(&mut self, addr: Addr) -> ProgramCounter {
        self.stack[self.sp] = self.pc + OPCODE_SIZE;
        self.sp += 1;
        ProgramCounter::Jump(addr as usize)
    }

    fn op_skip_equal_k(&mut self, x: usize, k: Byte) -> ProgramCounter {
        ProgramCounter::skip_if(self.v[x] == k)
    }

    fn op_skip_not_equal_k(&mut self, x: usize, k: Byte) -> ProgramCounter {
        ProgramCounter::skip_if(self.v[x] != k)
    }

    fn op_skip_equal(&mut self, x: usize, y: usize) -> ProgramCounter {
        ProgramCounter::skip_if(self.v[x] == self.v[y])
    }

    fn op_load_k(&mut self, x: usize, k: Byte) -> ProgramCounter {
        self.v[x] = k;
        ProgramCounter::Next
    }

    fn op_add_k(&mut self, x: usize, k: Byte) -> ProgramCounter {
        self.v[x] = ((self.v[x] as u16) + (k as u16)) as u8;
        ProgramCounter::Next
    }

    fn op_set(&mut self, x: usize, y: usize) -> ProgramCounter {
        self.v[x] = self.v[y];
        ProgramCounter::Next
    }

    fn op_or(&mut self, x: usize, y: usize) -> ProgramCounter {
        self.v[x] |= self.v[y];
        ProgramCounter::Next
    }

    fn op_and(&mut self, x: usize, y: usize) -> ProgramCounter {
        self.v[x] &= self.v[y];
        ProgramCounter::Next
    }

    fn op_xor(&mut self, x: usize, y: usize) -> ProgramCounter {
        self.v[x] ^= self.v[y];
        ProgramCounter::Next
    }

    fn op_add(&mut self, x: usize, y: usize) -> ProgramCounter {
        let r = (self.v[x] as u16) + (self.v[y] as u16);
        self.v[x] = r as u8;
        self.v[0xf] = if r > 0xff { 1 } else { 0 };
        ProgramCounter::Next
    }

    fn op_sub(&mut self, x: usize, y: usize) -> ProgramCounter {
        self.v[0xf] = if self.v[x] > self.v[y] { 1 } else { 0 };
        self.v[x] = self.v[x].wrapping_sub(self.v[y]);
        ProgramCounter::Next
    }

    fn op_shift_right(&mut self, x: usize) -> ProgramCounter {
        self.v[0xf] = self.v[x] & 1;
        self.v[x] >>= 1;
        ProgramCounter::Next
    }

    fn op_sub_inv(&mut self, x: usize, y: usize) -> ProgramCounter {
        self.v[0xf] = if self.v[y] > self.v[x] { 1 } else { 0 };
        self.v[x] = self.v[y].wrapping_sub(self.v[x]);
        ProgramCounter::Next
    }

    fn op_shift_left(&mut self, x: usize) -> ProgramCounter {
        self.v[0xf] = (self.v[x] & 0b10000000) >> 7;
        self.v[x] <<= 1;
        ProgramCounter::Next
    }

    fn op_skip_not_equal(&mut self, x: usize, y: usize) -> ProgramCounter {
        ProgramCounter::skip_if(self.v[x] != self.v[y])
    }

    fn op_load_i(&mut self, addr: Addr) -> ProgramCounter {
        self.i = addr as u16;
        ProgramCounter::Next
    }

    fn op_long_jump(&mut self, addr: Addr) -> ProgramCounter {
        ProgramCounter::Jump((self.v[0] as usize) + (addr as usize))
    }

    fn op_rand(&mut self, x: usize, kk: Byte) -> ProgramCounter {
        let rn = rand::random::<u8>();
        self.v[x] = rn & kk;
        ProgramCounter::Next
    }

    fn op_draw(&mut self, x: usize, y: usize, n: usize) -> ProgramCounter {
        self.v[0x0f] = 0;
        for byte in 0..n {
            let sy = (self.v[y] as usize + byte) % CHIP8_HEIGHT;
            for bit in 0..8 {
                let sx = (self.v[x] as usize + bit) % CHIP8_WIDTH;
                let color = (self.ram[self.i as usize + byte as usize] >> (7 - bit)) & 1;
                self.v[0xf] |= color & self.vram[sy][sx];
                self.vram[sy][sx] ^= color;
            }
        }
        self.vram_changed = true;
        ProgramCounter::Next
    }

    fn op_skip_pressed(&mut self, x: usize) -> ProgramCounter {
        ProgramCounter::skip_if(self.keypad[self.v[x] as usize])
    }

    fn op_skip_not_pressed(&mut self, x: usize) -> ProgramCounter {
        ProgramCounter::skip_if(! self.keypad[self.v[x] as usize])
    }

    fn op_get_timer(&mut self, x: usize) -> ProgramCounter {
        self.v[x] = self.delay_timer;
        ProgramCounter::Next
    }

    fn op_wait_key(&mut self, x: usize) -> ProgramCounter {
        self.keypad_waiting = true;
        self.keypad_register = x;
        ProgramCounter::Next
    }

    fn op_set_timer(&mut self, x: usize) -> ProgramCounter {
        self.delay_timer = self.v[x];
        ProgramCounter::Next
    }

    fn op_set_sound_timer(&mut self, x: usize) -> ProgramCounter {
        self.sound_timer = self.v[x];
        ProgramCounter::Next
    }

    fn op_add_i(&mut self, x: usize) -> ProgramCounter {
        let n: usize = self.i as usize + self.v[x] as usize;
        self.i = n as u16;
        self.v[0xf] = if n > 0x0F00 { 1 } else { 0 };
        ProgramCounter::Next
    }

    fn op_load_hex_glyph(&mut self, x: usize) -> ProgramCounter {
        self.i = (self.v[x] as u16) * 5;
        ProgramCounter::Next
   }

    fn op_store_bcd(&mut self, x: usize) -> ProgramCounter {
        let i: usize = self.i as usize;
        self.ram[i] = self.v[x] / 100;
        self.ram[i + 1] = (self.v[x] % 100) / 10;
        self.ram[i + 2] = self.v[x] % 10;
        ProgramCounter::Next
    }

    fn op_store_registers(&mut self, x: usize) -> ProgramCounter {
        for i in 0..x+1 {
            self.ram[self.i as usize + i] = self.v[i];
        }
        ProgramCounter::Next
    }

    fn op_load_registers(&mut self, x: usize) -> ProgramCounter {
        for i in 0..x+1 {
            self.v[i] = self.ram[self.i as usize + i];
        }
        ProgramCounter::Next
    }
}