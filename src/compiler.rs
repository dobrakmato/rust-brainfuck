use std::io::{stdin, Read};
use memmap::MmapMut;
use crate::ir::{IrCode, IrOp};
use crate::brainfuck::MAX_MEMORY;
use crate::assembler::{Assembler, X64Register};

/* Brainfuck Read and Write procedures. */
extern "win64" fn putchar(character: u8) {
    print!("{}", character as char);
}

extern "win64" fn getchar() -> u8 {
    let mut buff: [u8; 1] = [0; 1];
    stdin().read_exact(&mut buff).expect("cannot read from stdin");
    buff[0]
}

const PUTCHAR_REGISTER: X64Register = X64Register::R12;
const GETCHAR_REGISTER: X64Register = X64Register::R13;
const PTR_REGISTER: X64Register = X64Register::R14;

pub struct IoFn {
    putchar_ptr: usize,
    getchar_ptr: usize,
}

impl IoFn {
    pub fn std() -> Self {
        return IoFn {
            putchar_ptr: putchar as usize,
            getchar_ptr: getchar as usize,
        };
    }
}

impl IrCode {
    pub fn compile(&mut self, io_fn: IoFn) -> Brainfuck {
        let length = self.len();

        let mut brainfuck = Brainfuck::new(256 + length * 16);
        let mut assembler: Assembler = Assembler::new(&mut brainfuck.program);

        assembler.push(X64Register::RBX);
        assembler.push(PUTCHAR_REGISTER);
        assembler.push(GETCHAR_REGISTER);
        assembler.push(PTR_REGISTER);
        assembler.sub(X64Register::RSP, 168);

        assembler.mov(PUTCHAR_REGISTER, io_fn.putchar_ptr as u64);
        assembler.mov(GETCHAR_REGISTER, io_fn.getchar_ptr as u64);
        assembler.mov(PTR_REGISTER, brainfuck.memory.as_ptr() as u64);

        let mut parentheses_depth = 0usize;
        let mut parentheses_id_stack = [0; 4096];

        /* 1. generate instructions */
        for op in self.iter() {
            match op {
                IrOp::Noop(_) => {}
                IrOp::Right(_, data) => assembler.add(PTR_REGISTER, (*data).into()),
                IrOp::Left(_, data) => assembler.sub(PTR_REGISTER, (*data).into()),
                IrOp::Add(_, data) => assembler.add_indirect(PTR_REGISTER, *data),
                IrOp::Sub(_, data) => assembler.sub_indirect(PTR_REGISTER, *data),
                IrOp::SetIndirect(_, data) => assembler.mov_indirect(PTR_REGISTER, *data),
                IrOp::MulCopy(_, offset, factor) => {
                    assembler.mov_to_reg(X64Register::RAX, PTR_REGISTER);
                    assembler.mov(X64Register::RBX, *factor as u64);
                    if *factor != 1 {
                        assembler.mul_signed(X64Register::RBX);
                    }
                    assembler.add_to_mem_offset(PTR_REGISTER, X64Register::RAX, *offset)
                }
                IrOp::Write(_) => {
                    assembler.mov_to_reg(X64Register::RCX, PTR_REGISTER);
                    assembler.call(PUTCHAR_REGISTER);
                }
                IrOp::Read(_) => {
                    assembler.call(GETCHAR_REGISTER);
                    assembler.mov_to_memory(PTR_REGISTER, X64Register::RAX);
                }
                IrOp::JumpIfZero(_, _) => {
                    parentheses_depth += 1;
                    parentheses_id_stack[parentheses_depth] += 1;

                    assembler.label(format!("[{}_{}", parentheses_depth, parentheses_id_stack[parentheses_depth]));
                    assembler.cmp_indirect(PTR_REGISTER, 0);
                    assembler.je(0x00AA_BBCC);
                }
                IrOp::JumpIfNotZero(_, _) => {
                    assembler.cmp_indirect(PTR_REGISTER, 0);
                    assembler.jne_label(format!("[{}_{}", parentheses_depth, parentheses_id_stack[parentheses_depth]));
                    assembler.label(format!("]{}_{}", parentheses_depth, parentheses_id_stack[parentheses_depth]));
                    parentheses_depth -= 1;
                }
            }
        }

        assembler.add(X64Register::RSP, 168);
        assembler.pop(PTR_REGISTER);
        assembler.pop(GETCHAR_REGISTER);
        assembler.pop(PUTCHAR_REGISTER);
        assembler.pop(X64Register::RBX);

        assembler.ret();

        /* save actual program length */
        brainfuck.length = assembler.addr;

        /* 2. resolve jumps */
        let jumps_to_fix: Vec<(String, usize)> = assembler.labels
            .iter()
            .filter(|(k, _)| k.starts_with('['))
            .map(|(k, v)| (k.clone(), *v))
            .collect();

        for (k, v) in jumps_to_fix {
            assembler.addr = v;
            assembler.cmp_indirect(PTR_REGISTER, 0);
            assembler.je_label(k.replace('[', "]"));
        }

        brainfuck
    }
}

pub struct Brainfuck {
    pub program: MmapMut,
    pub length: usize,
    memory: [u8; MAX_MEMORY],
}

impl Brainfuck {
    fn new(size: usize) -> Self {
        let mut binary = MmapMut::map_anon(size).expect("cannot allocate memory");

        /* fill memory with INT3 for debugging */
        binary.iter_mut().for_each(|x| *x = 0xCCu8);

        Brainfuck {
            program: binary,
            length: 0,
            memory: [0; MAX_MEMORY],
        }
    }

    pub extern "C" fn execute(self) {
        let executable = self.program.make_exec().expect("cannot make memory executable");
        let ptr = executable.as_ptr() as *const ();
        let compiled_brainfuck: extern "C" fn() = unsafe { std::mem::transmute(ptr) };

        compiled_brainfuck();
    }
}

#[cfg(test)]
mod test {
    use crate::ir::{IrCode, IrOp};
    use crate::brainfuck::Program;
    use crate::compiler::{IoFn, getchar};

    #[test]
    fn does_not_crash() {
        let mut ir_code = IrCode { ops: vec![IrOp::Noop(None)] };
        let brainfuck = ir_code.compile(IoFn::std());

        brainfuck.execute();
    }

    static mut OUTPUT: [u8; 4096] = [0; 4096];
    static mut OUTPUT_IDX: usize = 0;

    extern "win64" fn value_putchar(character: u8) {
        unsafe {
            OUTPUT[OUTPUT_IDX] = character;
            OUTPUT_IDX += 1;
        };
    }

    #[test]
    fn copy_multiplied() {
        let op1 = IrOp::SetIndirect(Some(1), 7);
        let op2 = IrOp::MulCopy(Some(2), 2, 11);
        let op3 = IrOp::Right(Some(3), 2);
        let op4 = IrOp::Write(None);

        let mut ir_code = IrCode { ops: vec![op1, op2, op3, op4] };
        let brainfuck = ir_code.compile(IoFn { putchar_ptr: value_putchar as usize, getchar_ptr: getchar as usize });

        unsafe { OUTPUT_IDX = 0; }

        brainfuck.execute();

        assert_eq!(unsafe { OUTPUT[0] }, b'M');
    }

    #[test]
    fn can_run_pi_bf() {
        let pi_program = Program::from_string(">  +++++ +++++ +++++
[<+>>>>>>>>++++++++++<<<<<<<-]>+++++[<+++++++++>-]+>>>>>>+[<<+++[>>[-<]<[>]<-]>>
[>+>]<[<]>]>[[->>>>+<<<<]>>>+++>-]<[<<<<]<<<<<<<<+[->>>>>>>>>>>>[<+[->>>>+<<<<]>
>>>>]<<<<[>>>>>[<<<<+>>>>-]<<<<<-[<<++++++++++>>-]>>>[<<[<+<<+>>>-]<[>+<-]<++<<+
>>>>>>-]<<[-]<<-<[->>+<-[>>>]>[[<+>-]>+>>]<<<<<]>[-]>+<<<-[>>+<<-]<]<<<<+>>>>>>>
>[-]>[<<<+>>>-]<<++++++++++<[->>+<-[>>>]>[[<+>-]>+>>]<<<<<]>[-]>+>[<<+<+>>>-]<<<
<+<+>>[-[-[-[-[-[-[-[-[-<->[-<+<->>]]]]]]]]]]<[+++++[<<<++++++++<++++++++>>>>-]<
<<<+<->>>>[>+<<<+++++++++<->>>-]<<<<<[>>+<<-]+<[->-<]>[>>.<<<<[+.[-]]>>-]>[>>.<<
-]>[-]>[-]>>>[>>[<<<<<<<<+>>>>>>>>-]<<-]]>>[-]<<<[-]<<<<<<<<]++++++++++.");
        let mut ir_code = IrCode::new(&pi_program);
        let brainfuck = ir_code.compile(IoFn { putchar_ptr: value_putchar as usize, getchar_ptr: getchar as usize });

        unsafe { OUTPUT_IDX = 0; }

        brainfuck.execute();

        assert_eq!(unsafe { OUTPUT[0] }, b'3');
        assert_eq!(unsafe { OUTPUT[1] }, b'.');
        assert_eq!(unsafe { OUTPUT[2] }, b'1');
        assert_eq!(unsafe { OUTPUT[3] }, b'4');
        assert_eq!(unsafe { OUTPUT[4] }, b'0');
        assert_eq!(unsafe { OUTPUT[5] }, b'7');
        assert_eq!(unsafe { OUTPUT[6] }, b'0');
        assert_eq!(unsafe { OUTPUT[7] }, b'4');
        assert_eq!(unsafe { OUTPUT[8] }, b'5');
        assert_eq!(unsafe { OUTPUT[9] }, b'5');
        assert_eq!(unsafe { OUTPUT[10] }, b'2');
        assert_eq!(unsafe { OUTPUT[11] }, b'8');
        assert_eq!(unsafe { OUTPUT[12] }, b'2');
        assert_eq!(unsafe { OUTPUT[13] }, b'8');
        assert_eq!(unsafe { OUTPUT[14] }, b'8');
        assert_eq!(unsafe { OUTPUT[15] }, b'5');
    }
}