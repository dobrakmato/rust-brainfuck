use std::collections::HashMap;
use bitflags::bitflags;

#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
pub enum X64Register {
    RAX = 0,
    RCX = 1,
    RDX = 2,
    RBX = 3,
    RSP = 4,
    RBP = 5,
    RSI = 6,
    RDI = 7,
    R8 = 8,
    R9 = 9,
    R10 = 10,
    R11 = 11,
    R12 = 12,
    R13 = 13,
    R14 = 14,
    R15 = 15,
}

impl X64Register {
    fn is_extended(self) -> bool {
        self as u8 > 7
    }

    fn to_u8(self) -> u8 {
        self as u8 & 7 // wrap
    }
}

pub struct Assembler<'a> {
    pub data: &'a mut [u8],
    pub addr: usize,
    pub labels: HashMap<String, usize>,
}

impl<'a> Assembler<'a> {
    pub fn new(data: &'a mut [u8]) -> Self {
        Assembler {
            data,
            addr: 0,
            labels: HashMap::new(),
        }
    }

    fn put(&mut self, value: u8) {
        self.data[self.addr] = value;
        self.addr += 1;
    }

    fn imm32(&mut self, imm: u32) {
        self.put((imm & 0xFF) as u8);
        self.put(((imm >> 8) & 0xFF) as u8);
        self.put(((imm >> 16) & 0xFF) as u8);
        self.put(((imm >> 24) & 0xFF) as u8);
    }

    fn imm64(&mut self, imm: u64) {
        self.put((imm & 0xFF) as u8);
        self.put(((imm >> 8) & 0xFF) as u8);
        self.put(((imm >> 16) & 0xFF) as u8);
        self.put(((imm >> 24) & 0xFF) as u8);
        self.put(((imm >> 32) & 0xFF) as u8);
        self.put(((imm >> 40) & 0xFF) as u8);
        self.put(((imm >> 48) & 0xFF) as u8);
        self.put(((imm >> 56) & 0xFF) as u8);
    }

    fn mod_rm(&mut self, reg_opcode: u8, r#mod: u8, rm: u8) {
        let value = (r#mod << 6) | (reg_opcode << 3) | rm;
        self.put(value);
    }

    fn sib(&mut self, base: u8, scale: u8, index: u8) {
        let value = (scale << 6) | (index << 3) | base;
        self.put(value);
    }

    /* instructions */

    pub fn mov(&mut self, reg: X64Register, imm: u64) {
        let rex = Rex::W | if reg.is_extended() { Rex::B } else { Rex::empty() };

        self.put(rex.bits());
        self.put(0xB8 + reg.to_u8());
        self.imm64(imm);
    }

    pub fn add(&mut self, reg: X64Register, imm: u32) {
        let rex = Rex::W | if reg.is_extended() { Rex::B } else { Rex::empty() };

        self.put(rex.bits());
        self.put(0x81);
        self.mod_rm(0, 0b11, reg.to_u8());
        self.imm32(imm);
    }

    pub fn sub(&mut self, reg: X64Register, imm: u32) {
        let rex = Rex::W | if reg.is_extended() { Rex::B } else { Rex::empty() };

        self.put(rex.bits());
        self.put(0x81);
        self.mod_rm(5, 0b11, reg.to_u8());
        self.imm32(imm);
    }

    fn op_80(&mut self, opcode: u8, memory: X64Register, imm: u8) {
        if memory.is_extended() {
            self.put(Rex::B.bits());
        }
        self.put(0x80);
        match memory {
            X64Register::R12 => {
                self.mod_rm(opcode, 0b00, 4);
                self.sib(4, 0, 4);
            }
            X64Register::R13 => {
                self.mod_rm(opcode, 0b01, 0b101);
                self.put(0x00); // +0 (+disp8)
            }
            _ => {
                self.mod_rm(opcode, 0b00, memory.to_u8());
            }
        }
        self.put(imm);
    }

    pub fn add_indirect(&mut self, memory: X64Register, imm: u8) {
        self.op_80(0, memory, imm);
    }

    pub fn sub_indirect(&mut self, memory: X64Register, imm: u8) {
        self.op_80(5, memory, imm);
    }

    pub fn cmp_indirect(&mut self, memory: X64Register, imm: u8) {
        self.op_80(7, memory, imm);
    }

    pub fn mov_to_reg(&mut self, to: X64Register, from_memory: X64Register) {
        let rex = Rex::W | if from_memory.is_extended() { Rex::B } else { Rex::empty() };
        let rex = rex | if to.is_extended() { Rex::R } else { Rex::empty() };

        self.put(rex.bits());
        self.put(0x0F);
        self.put(0xB6);

        match from_memory {
            X64Register::R12 => {
                self.mod_rm(to.to_u8(), 0b00, 4);
                self.sib(4, 0, 4);
            }
            X64Register::R13 => {
                self.mod_rm(to.to_u8(), 0b01, 0b101);
                self.put(0x00); // +0 (+disp8)
            }
            _ => {
                self.mod_rm(to.to_u8(), 0b00, from_memory.to_u8());
            }
        }
    }

    pub fn mov_to_memory(&mut self, to_memory: X64Register, from_reg: X64Register) {
        let rex = if to_memory.is_extended() { Rex::B } else { Rex::empty() };
        let rex = rex | if from_reg.is_extended() { Rex::R } else { Rex::empty() };

        if !rex.is_empty() {
            self.put(rex.bits());
        }

        self.put(0x88);

        match to_memory {
            X64Register::R12 => {
                self.mod_rm(from_reg.to_u8(), 0b00, 4);
                self.sib(4, 0, 4);
            }
            X64Register::R13 => {
                self.mod_rm(from_reg.to_u8(), 0b01, 0b101);
                self.put(0x00); // +0 (+disp8)
            }
            _ => {
                self.mod_rm(from_reg.to_u8(), 0b00, to_memory.to_u8());
            }
        }
    }

    pub fn je(&mut self, relative_addr: i32) {
        self.put(0x0f);
        self.put(0x84);
        self.imm32(relative_addr as u32)
    }

    pub fn jne(&mut self, relative_addr: i32) {
        self.put(0x0f);
        self.put(0x85);
        self.imm32(relative_addr as u32)
    }

    pub fn jne_label(&mut self, label: String) {
        let label_addr = *self.labels.get(&label).expect("label does not exists") as i32;
        let relative_addr = label_addr - (self.addr as i32 + 6);
        self.jne(relative_addr);
    }

    pub fn je_label(&mut self, label: String) {
        let label_addr = *self.labels.get(&label).expect("label does not exists") as i32;
        let relative_addr = label_addr - (self.addr as i32 + 6);
        self.je(relative_addr);
    }


    pub fn label(&mut self, label: String) {
        self.labels.insert(label, self.addr);
    }

    pub fn call(&mut self, reg: X64Register) {
        if reg.is_extended() {
            self.put(Rex::B.bits());
        }
        self.put(0xff);
        self.mod_rm(2, 11, reg.to_u8());
    }

    pub fn ret(&mut self) {
        self.put(0xC3);
    }
}

bitflags! {
    struct Rex: u8 {
        const B = 0b0100_0001; // This 1-bit value is an extension to the MODRM.rm field or the SIB.base field. See 64-bit addressing.
        const X = 0b0100_0010; // This 1-bit value is an extension to the SIB.index field. See 64-bit addressing.
        const R = 0b0100_0100; // This 1-bit value is an extension to the MODRM.reg field. See Registers.
        const W = 0b0100_1000; // When 1, a 64-bit operand size is used. Otherwise, when 0, the default operand size is used (which is 32-bit for most but not all instructions, see this table).
    }
}


#[cfg(test)]
mod test {
    use std::collections::HashMap;
    use crate::assembler::{Assembler, X64Register};

    #[test]
    fn mov() {
        let mut asm = Assembler { addr: 0, data: &mut [0; 32], labels: HashMap::new() };

        // 48 b8 ef be ad de ef be ad de    movabs rax,0xdeadbeefdeadbeef
        asm.mov(X64Register::RAX, 0xdead_beef_dead_beef);
        assert_eq!(asm.data[..10], [0x48, 0xb8, 0xef, 0xbe, 0xad, 0xde, 0xef, 0xbe, 0xad, 0xde]);
        asm.addr = 0;

        // 48 bb ef be ad de ef be ad de    movabs rbx,0xdeadbeefdeadbeef
        asm.mov(X64Register::RBX, 0xdead_beef_dead_beef);
        assert_eq!(asm.data[..10], [0x48, 0xbb, 0xef, 0xbe, 0xad, 0xde, 0xef, 0xbe, 0xad, 0xde]);
        asm.addr = 0;

        // 49 bc ef be ad de ef be ad de    movabs r12,0xdeadbeefdeadbeef
        asm.mov(X64Register::R12, 0xdead_beef_dead_beef);
        assert_eq!(asm.data[..10], [0x49, 0xbc, 0xef, 0xbe, 0xad, 0xde, 0xef, 0xbe, 0xad, 0xde]);
        asm.addr = 0;
    }

    #[test]
    fn add() {
        let mut asm = Assembler { addr: 0, data: &mut [0; 32], labels: HashMap::new() };

        // 48 81 c2 cd ab 00 00    add    rdx,0xabcd
        asm.add(X64Register::RDX, 0xabcd);
        assert_eq!(asm.data[..7], [0x48, 0x81, 0xc2, 0xcd, 0xab, 0x00, 0x00]);
        asm.addr = 0;

        // 49 81 c4 cd ab 00 00    add    r12,0xabcd
        asm.add(X64Register::R12, 0xabcd);
        assert_eq!(asm.data[..7], [0x49, 0x81, 0xc4, 0xcd, 0xab, 0x00, 0x00]);
        asm.addr = 0;
    }

    #[test]
    fn sub() {
        let mut asm = Assembler { addr: 0, data: &mut [0; 32], labels: HashMap::new() };

        // 48 81 ea cd ab 00 00    sub    rdx,0xabcd
        asm.sub(X64Register::RDX, 0xabcd);
        assert_eq!(asm.data[..7], [0x48, 0x81, 0xea, 0xcd, 0xab, 0x00, 0x00]);
        asm.addr = 0;

        // 49 81 ec cd ab 00 00    sub    r12,0xabcd
        asm.sub(X64Register::R12, 0xabcd);
        assert_eq!(asm.data[..7], [0x49, 0x81, 0xec, 0xcd, 0xab, 0x00, 0x00]);
        asm.addr = 0;
    }

    #[test]
    fn add_indirect() {
        let mut asm = Assembler { addr: 0, data: &mut [0; 32], labels: HashMap::new() };

        // 80 02 ab                add    BYTE PTR [rdx],0xab
        asm.add_indirect(X64Register::RDX, 0xab);
        assert_eq!(asm.data[..3], [0x80, 0x02, 0xab]);
        asm.addr = 0;

        // 41 80 00 ab             add    BYTE PTR [r8],0xab
        asm.add_indirect(X64Register::R8, 0xab);
        assert_eq!(asm.data[..4], [0x41, 0x80, 0x00, 0xab]);
        asm.addr = 0;

        // 41 80 04 24 ab          add    BYTE PTR [r12],0xab
        asm.add_indirect(X64Register::R12, 0xab);
        assert_eq!(asm.data[..5], [0x41, 0x80, 0x04, 0x24, 0xab]);
        asm.addr = 0;

        // 41 80 45 00 ab          add    BYTE PTR [r13+0x0],0xab
        asm.add_indirect(X64Register::R13, 0xab);
        assert_eq!(asm.data[..5], [0x41, 0x80, 0x45, 0x00, 0xab]);
        asm.addr = 0;

        // 41 80 06 ab             add    BYTE PTR [r14],0xab
        asm.add_indirect(X64Register::R14, 0xab);
        assert_eq!(asm.data[..4], [0x41, 0x80, 0x06, 0xab]);
        asm.addr = 0;
    }


    #[test]
    fn sub_indirect() {
        let mut asm = Assembler { addr: 0, data: &mut [0; 32], labels: HashMap::new() };

        // 80 2a ab                sub    BYTE PTR [rdx],0xab
        asm.sub_indirect(X64Register::RDX, 0xab);
        assert_eq!(asm.data[..3], [0x80, 0x2a, 0xab]);
        asm.addr = 0;

        // 41 80 28 ab             sub    BYTE PTR [r8],0xab
        asm.sub_indirect(X64Register::R8, 0xab);
        assert_eq!(asm.data[..4], [0x41, 0x80, 0x28, 0xab]);
        asm.addr = 0;

        // 41 80 2c 24 ab          sub    BYTE PTR [r12],0xab
        asm.sub_indirect(X64Register::R12, 0xab);
        assert_eq!(asm.data[..5], [0x41, 0x80, 0x2c, 0x24, 0xab]);
        asm.addr = 0;

        // 41 80 6d 00 ab          sub    BYTE PTR [r13+0x0],0xab
        asm.sub_indirect(X64Register::R13, 0xab);
        assert_eq!(asm.data[..5], [0x41, 0x80, 0x6d, 0x00, 0xab]);
        asm.addr = 0;

        // 41 80 2e ab             sub    BYTE PTR [r14],0xab
        asm.sub_indirect(X64Register::R14, 0xab);
        assert_eq!(asm.data[..4], [0x41, 0x80, 0x2e, 0xab]);
        asm.addr = 0;
    }

    #[test]
    fn cmp_indirect() {
        let mut asm = Assembler { addr: 0, data: &mut [0; 32], labels: HashMap::new() };

        // 80 3a ab                cmp    BYTE PTR [rdx],0xab
        asm.cmp_indirect(X64Register::RDX, 0xab);
        assert_eq!(asm.data[..3], [0x80, 0x3a, 0xab]);
        asm.addr = 0;

        // 41 80 38 ab             cmp    BYTE PTR [r8],0xab
        asm.cmp_indirect(X64Register::R8, 0xab);
        assert_eq!(asm.data[..4], [0x41, 0x80, 0x38, 0xab]);
        asm.addr = 0;

        // 41 80 3c 24 ab          cmp    BYTE PTR [r12],0xab
        asm.cmp_indirect(X64Register::R12, 0xab);
        assert_eq!(asm.data[..5], [0x41, 0x80, 0x3c, 0x24, 0xab]);
        asm.addr = 0;

        // 41 80 7d 00 ab          cmp    BYTE PTR [r13+0x0],0xab
        asm.cmp_indirect(X64Register::R13, 0xab);
        assert_eq!(asm.data[..5], [0x41, 0x80, 0x7d, 0x00, 0xab]);
        asm.addr = 0;

        // 41 80 3e ab             cmp    BYTE PTR [r14],0xab
        asm.cmp_indirect(X64Register::R14, 0xab);
        assert_eq!(asm.data[..4], [0x41, 0x80, 0x3e, 0xab]);
        asm.addr = 0;
    }

    #[test]
    fn mov_to_reg() {
        let mut asm = Assembler { addr: 0, data: &mut [0; 32], labels: HashMap::new() };

        // 49 0f b6 01             movzx  rax,BYTE PTR [r9]
        asm.mov_to_reg(X64Register::RAX, X64Register::R9);
        assert_eq!(asm.data[..4], [0x49, 0x0f, 0xb6, 0x01]);
        asm.addr = 0;

        // 48 0f b6 03             movzx  rax,BYTE PTR [rbx]
        asm.mov_to_reg(X64Register::RAX, X64Register::RBX);
        assert_eq!(asm.data[..4], [0x48, 0x0f, 0xb6, 0x03]);
        asm.addr = 0;

        // 4d 0f b6 08             movzx  r9,BYTE PTR [r8]
        asm.mov_to_reg(X64Register::R9, X64Register::R8);
        assert_eq!(asm.data[..4], [0x4d, 0x0f, 0xb6, 0x08]);
        asm.addr = 0;

        // 49 0f b6 04 24          movzx  rax,BYTE PTR [r12]
        asm.mov_to_reg(X64Register::RAX, X64Register::R12);
        assert_eq!(asm.data[..5], [0x49, 0x0f, 0xb6, 0x04, 0x24]);
        asm.addr = 0;

        // 4d 0f b6 20             movzx  r12,BYTE PTR [r8]
        asm.mov_to_reg(X64Register::R12, X64Register::R8);
        assert_eq!(asm.data[..4], [0x4d, 0x0f, 0xb6, 0x20]);
        asm.addr = 0;

        // 49 0f b6 45 00          movzx  rax,BYTE PTR [r13+0x0]
        asm.mov_to_reg(X64Register::RAX, X64Register::R13);
        assert_eq!(asm.data[..5], [0x49, 0x0f, 0xb6, 0x45, 0x00]);
        asm.addr = 0;

        // 4c 0f b6 2a             movzx  r13,BYTE PTR [rdx]
        asm.mov_to_reg(X64Register::R13, X64Register::RDX);
        assert_eq!(asm.data[..4], [0x4c, 0x0f, 0xb6, 0x2a]);
        asm.addr = 0;
    }

    #[test]
    fn mov_to_memory() {
        let mut asm = Assembler { addr: 0, data: &mut [0; 32], labels: HashMap::new() };

        // 88 03                   mov    BYTE PTR [rbx],al
        asm.mov_to_memory(X64Register::RBX, X64Register::RAX);
        assert_eq!(asm.data[..2], [0x88, 0x03]);
        asm.addr = 0;

        // 41 88 00                mov    BYTE PTR [r8],al
        asm.mov_to_memory(X64Register::R8, X64Register::RAX);
        assert_eq!(asm.data[..3], [0x41, 0x88, 0x00]);
        asm.addr = 0;

        // 41 88 04 24             mov    BYTE PTR [r12],al
        asm.mov_to_memory(X64Register::R12, X64Register::RAX);
        assert_eq!(asm.data[..4], [0x41, 0x88, 0x04, 0x24]);
        asm.addr = 0;

        // 41 88 45 00             mov    BYTE PTR [r13+0x0],al
        asm.mov_to_memory(X64Register::R13, X64Register::RAX);
        assert_eq!(asm.data[..4], [0x41, 0x88, 0x45, 0x00]);
        asm.addr = 0;

        // 44 88 03                mov    BYTE PTR [rbx],r8b
        asm.mov_to_memory(X64Register::RBX, X64Register::R8);
        assert_eq!(asm.data[..3], [0x44, 0x88, 0x03]);
        asm.addr = 0;

        // 45 88 04 24             mov    BYTE PTR [r12],r8b
        asm.mov_to_memory(X64Register::R12, X64Register::R8);
        assert_eq!(asm.data[..4], [0x45, 0x88, 0x04, 0x24]);
        asm.addr = 0;

        // 45 88 45 00             mov    BYTE PTR [r13+0x0],r8b
        asm.mov_to_memory(X64Register::R13, X64Register::R8);
        assert_eq!(asm.data[..4], [0x45, 0x88, 0x45, 0x00]);
        asm.addr = 0;
    }

    #[test]
    fn je() {
        let mut asm = Assembler { addr: 0, data: &mut [0; 32], labels: HashMap::new() };

        asm.je(0x0A0A_0B0B);
        assert_eq!(asm.data[..6], [0x0f, 0x84, 0x0b, 0x0b, 0x0a, 0x0a]);
    }

    #[test]
    fn jne() {
        let mut asm = Assembler { addr: 0, data: &mut [0; 32], labels: HashMap::new() };

        asm.jne(0x0A0A_0B0B);
        assert_eq!(asm.data[..6], [0x0f, 0x85, 0x0b, 0x0b, 0x0a, 0x0a]);
    }

    #[test]
    fn call() {
        let mut asm = Assembler { addr: 0, data: &mut [0; 32], labels: HashMap::new() };

        // ff d3                   call   rbx
        asm.call(X64Register::RBX);
        assert_eq!(asm.data[..2], [0xff, 0xd3]);
        asm.addr = 0;

        // 41 ff d4                call   r12
        asm.call(X64Register::R12);
        assert_eq!(asm.data[..3], [0x41, 0xff, 0xd4]);
        asm.addr = 0;
    }


    #[test]
    fn ret() {
        let mut asm = Assembler { addr: 0, data: &mut [0; 32], labels: HashMap::new() };

        asm.ret();
        assert_eq!(asm.data[..1], [0xc3]);
    }
}

