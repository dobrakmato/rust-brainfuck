use std::time::Instant;
use crate::brainfuck::{Program, Op, MAX_MEMORY};
use crate::interpreter::Interpreter;
use clap::{App, Arg, ArgMatches};
use crate::ir::IrCode;
use crate::compiler::IoFn;

mod assembler;
mod ir;
mod compiler;
mod brainfuck;
mod interpreter;

#[cfg_attr(tarpaulin, skip)]
fn main() {
    let matches = App::new("bfjit")
        .version("v1.0")
        .author("Matej Kormuth <matej.kormuth@gmail.com>")
        .arg(Arg::with_name("interpreter")
            .short("i")
            .long("interpreter")
            .help("Forces interpreter mode")
        )
        .arg(Arg::with_name("jit")
            .short("j")
            .long("jit")
            .help("Forces JIT x64 compiler mode")
        )
        .arg(Arg::with_name("dump")
            .short("d")
            .long("dump")
            .help("Dump intermediate representation of program")
        )
        .arg(Arg::with_name("unoptimize")
            .short("u")
            .long("unoptimize")
            .help("Disable brainfuck program optimization during IR stage")
        )
        .arg(Arg::with_name("INPUT")
            .required(true)
            .index(1)
            .help("Specified brainfuck source file to use")
            .takes_value(true)
        )
        .get_matches();


    let file = matches.value_of("INPUT").unwrap();
    let content = std::fs::read_to_string(file).expect("cannot read specified file");
    let program = Program::from_string(&content);

    let start = Instant::now();
    if matches.is_present("dump") {
        let mut ir_code = IrCode::new(&program);

        if !matches.is_present("unoptimize") {
            ir_code.optimize();
        }

        println!("{:?}", ir_code);
    } else if matches.is_present("interpreter") {
        interpreter(&program);
        println!("time={}ms (interpreter)", start.elapsed().as_millis())
    } else {
        let does_optimize = if matches.is_present("unoptimize") { "unoptimized" } else { "optimized" };
        jit(matches, &program);
        println!("time={}ms (jit; {})", start.elapsed().as_millis(), does_optimize)
    }
}

#[cfg_attr(tarpaulin, skip)]
fn jit(matches: ArgMatches, program: &Program) {
    let start = Instant::now();
    let mut ir_code = IrCode::new(&program);

    let unopt_len = ir_code.len();

    if !matches.is_present("unoptimize") {
        ir_code.optimize();
    }

    let opt_len = ir_code.len();

    let brainfuck = ir_code.compile(IoFn::std());
    println!("compile_time={}ms\tunopt={}\topt={}\tbytes={} of {} allocated ({:.2}% used)", start.elapsed().as_millis(),
             unopt_len, opt_len, brainfuck.length, brainfuck.program.len(), 100f32 * brainfuck.length as f32 / brainfuck.program.len() as f32);
    brainfuck.execute();
}

#[cfg_attr(tarpaulin, skip)]
fn interpreter(program: &Program) {
    let mut vm = Interpreter {
        program_counter: 0,
        program: &program,
        memory_pointer: 0,
        memory: [0; MAX_MEMORY],
        input: std::io::stdin(),
        output: std::io::stdout(),
    };
    vm.interpret();
}

