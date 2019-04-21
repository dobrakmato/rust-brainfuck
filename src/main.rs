use std::time::Instant;
use crate::brainfuck::{Program, Op, MAX_MEMORY};
use crate::interpreter::Interpreter;
use crate::ir::IrCode;

mod assembler;
mod ir;
mod compiler;
mod brainfuck;
mod interpreter;

fn main() {
    let file = std::env::args().nth(1).expect("no input file provided");
    let content = std::fs::read_to_string(file).expect("cannot read specified file");
    let program = Program::from_string(&content);

    // Create the VM with program, memory and registers.
    let mut vm = Interpreter {
        program_counter: 0,
        program: &program,
        memory_pointer: 0,
        memory: [0; MAX_MEMORY],
        input: std::io::stdin(),
        output: std::io::stdout(),
    };

    let start = Instant::now();
    vm.interpret();
    println!("interpreted time = {}ms", start.elapsed().as_millis());

    let start = Instant::now();
    let mut ir_code = IrCode::new(&program);
    ir_code.optimize();
    let compiled = ir_code.compile();
    println!("compilation time = {}ms", start.elapsed().as_millis());

    let start = Instant::now();
    compiled.execute();
    println!("compiled time = {}ms", start.elapsed().as_millis());
}

