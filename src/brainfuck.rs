/// Maximum memory in bytes an interpreter can use.
pub const MAX_MEMORY: usize = 30000;

pub enum Op {
    IncrementPtr,
    DecrementPtr,
    IncrementMemory,
    DecrementMemory,
    ReadByte,
    WriteByte,
    JumpForward,
    JumpBackward,
}

impl Op {
    fn from_char(character: char) -> Option<Self> {
        match character {
            '>' => Some(Op::IncrementPtr),
            '<' => Some(Op::DecrementPtr),
            '+' => Some(Op::IncrementMemory),
            '-' => Some(Op::DecrementMemory),
            '.' => Some(Op::WriteByte),
            ',' => Some(Op::ReadByte),
            '[' => Some(Op::JumpForward),
            ']' => Some(Op::JumpBackward),
            _ => None
        }
    }
}

pub struct Program {
    pub instructions: Vec<Op>,
}

impl Program {
    pub fn from_string(string: String) -> Self {
        let ops: Vec<Op> = string.chars()
            .map(|c| -> Option<Op> { Op::from_char(c) })
            .filter_map(|x| x)
            .collect();

        Program {
            instructions: ops,
        }
    }

    pub fn find_matching_jump_end(&self, jump_start_pos: usize) -> usize {
        let mut pos = jump_start_pos;
        let mut level = 0;

        loop {
            match self.instructions[pos] {
                Op::JumpForward => level += 1,
                Op::JumpBackward => level -= 1,
                _ => ()
            }

            if level == 0 { return pos; }
            if pos >= self.instructions.len() { panic!("unbalanced parentheses") }
            pos += 1
        }
    }

    pub fn find_matching_jump_start(&self, jump_end_pos: usize) -> usize {
        let mut pos = jump_end_pos;
        let mut level = 0;

        loop {
            match self.instructions[pos] {
                Op::JumpForward => level -= 1,
                Op::JumpBackward => level += 1,
                _ => ()
            }

            if level == 0 { return pos; }
            if pos == 0 { panic!("unbalanced parentheses") }
            pos -= 1
        }
    }
}