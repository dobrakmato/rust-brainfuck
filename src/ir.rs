use std::fmt::{Debug, Error, Formatter};
use crate::{Program, Op};

/// Link (aka. pointer) to next operation in program graph.
type Link = Option<usize>;

/// Operations in intermediate representation.
#[derive(Debug, Copy, Clone)]
pub enum IrOp {
    Noop(Link),
    Right(Link, u8),
    Left(Link, u8),
    Add(Link, u8),
    Sub(Link, u8),
    SetIndirect(Link, u8),
    Write(Link),
    Read(Link),
    /* next, addr if 0 */
    JumpIfZero(Link, Link),
    /* next, addr if not 0 */
    JumpIfNotZero(Link, Link),
}

impl IrOp {
    fn next(&self) -> Link {
        return match self {
            IrOp::Noop(l) => l,
            IrOp::Right(l, _) => l,
            IrOp::Left(l, _) => l,
            IrOp::Add(l, _) => l,
            IrOp::Sub(l, _) => l,
            IrOp::SetIndirect(l, _) => l,
            IrOp::Write(l) => l,
            IrOp::Read(l) => l,
            IrOp::JumpIfZero(l, _) => l,
            IrOp::JumpIfNotZero(l, _) => l,
        }.clone();
    }
}

/// Graph representation of program using intermediate representation with IrOps.
pub struct IrCode {
    pub ops: Vec<IrOp>
}

impl IrCode {
    pub fn new(program: &Program) -> Self {
        let mut ops: Vec<IrOp> = Vec::new();
        for (idx, op) in program.instructions.iter().enumerate() {
            let is_last = program.instructions.len() - 1 == idx;
            let next = if is_last { None } else { Some(idx + 1) };

            ops.push(match op {
                Op::IncrementPtr => IrOp::Right(next, 1),
                Op::DecrementPtr => IrOp::Left(next, 1),
                Op::IncrementMemory => IrOp::Add(next, 1),
                Op::DecrementMemory => IrOp::Sub(next, 1),
                Op::ReadByte => IrOp::Read(next),
                Op::WriteByte => IrOp::Write(next),
                Op::JumpForward => IrOp::JumpIfZero(next, Some(program.find_matching_jump_end(idx) + 1)),
                Op::JumpBackward => IrOp::JumpIfNotZero(next, Some(program.find_matching_jump_start(idx))),
            })
        }

        IrCode { ops }
    }

    fn find_replacement(&self, current_idx: usize) -> IrOp {
        let current = self.ops.get(current_idx).expect("current not found");
        let next_idx = match current.next() {
            Some(t) => t,
            None => return *current
        };
        let next = self.ops.get(next_idx).expect("next not found");
        let subsequent_idx = next.next();

        /* three consecutive ops */
        if let Some(t) = subsequent_idx {
            let subsequent = self.ops.get(t).expect("subsequent not found");
            let replacement = match (current, next, subsequent) {
                (IrOp::JumpIfZero(_, _), IrOp::Sub(_, 1), IrOp::JumpIfNotZero(far, _)) => Some(IrOp::SetIndirect(*far, 0)),
                (IrOp::JumpIfZero(_, _), IrOp::Add(_, 1), IrOp::JumpIfNotZero(far, _)) => Some(IrOp::SetIndirect(*far, 0)),
                _ => None,
            };

            if let Some(t) = replacement { return t; }
        }

        /* two consecutive ops */
        match (current, next) {
            (IrOp::Right(_, x), IrOp::Right(far, y)) => IrOp::Right(*far, *x + *y),
            (IrOp::Right(_, x), IrOp::Left(far, y)) => IrOp::Left(*far, *x - *y),
            (IrOp::Left(_, x), IrOp::Right(far, y)) => IrOp::Left(*far, *y - *x),
            (IrOp::Left(_, x), IrOp::Left(far, y)) => IrOp::Left(*far, *x + *y),

            (IrOp::Add(_, x), IrOp::Add(far, y)) => IrOp::Add(*far, *x + *y),
            (IrOp::Add(_, x), IrOp::Sub(far, y)) => IrOp::Sub(*far, *x - *y),
            (IrOp::Sub(_, x), IrOp::Add(far, y)) => IrOp::Sub(*far, *y - *x),
            (IrOp::Sub(_, x), IrOp::Sub(far, y)) => IrOp::Sub(*far, *x + *y),

            (c, _) => *c
        }
    }

    fn optimize_program_once(&mut self) -> usize {
        let mut idx = 0;
        let mut len = 0;

        loop {
            if idx == std::usize::MAX { return len; }

            let replacement = self.find_replacement(idx);
            let next_idx = match replacement.next() {
                Some(t) => t,
                None => std::usize::MAX,
            };
            self.ops[idx] = replacement;
            idx = next_idx;
            len += 1;
        }
    }

    pub fn optimize(&mut self) {
        let mut old = self.optimize_program_once();

        loop {
            let new = self.optimize_program_once();
            if new >= old { break; }
            old = new;
        }
    }

    pub fn iter(&self) -> Iter {
        Iter { ir_code: &self, idx: 0 }
    }

    // O(n)
    pub fn len(&self) -> usize {
        let mut idx = 0;
        let mut len = 0;

        loop {
            len += 1;
            let curr = self.ops.get(idx).expect("cannot get next instruction");
            idx = match curr.next() {
                Some(t) => t,
                None => return len
            };
        }
    }
}

pub struct Iter<'a> {
    ir_code: &'a IrCode,
    idx: usize,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a IrOp;

    fn next(&mut self) -> Option<Self::Item> {
        match self.ir_code.ops.get(self.idx) {
            Some(t) => {
                self.idx = t.next().unwrap_or(std::usize::MAX); // proceed or point to invalid idx
                Some(t)
            }
            None => None,
        }
    }
}

impl Debug for IrCode {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        let mut current = self.ops.get(0);

        f.write_str("IrCode {\n")?;

        loop {
            if current.is_none() { break; }

            let next = current.unwrap().next();

            f.write_fmt(format_args!("\t{:?},\n", current))?;
            current = next.and_then(|x| self.ops.get(x));
        }

        f.write_str("}\n")?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::ir::{IrCode, IrOp};
    use crate::brainfuck::Program;
    use matches::assert_matches;

    #[test]
    fn iter() {
        let ir_code = IrCode::new(&Program::from_string("+-<>"));
        let mut iter = ir_code.iter();

        assert_matches!(iter.next(), Some(IrOp::Add(_, 1)));
        assert_matches!(iter.next(), Some(IrOp::Sub(_, 1)));
        assert_matches!(iter.next(), Some(IrOp::Left(_, 1)));
        assert_matches!(iter.next(), Some(IrOp::Right(_, 1)));
        assert_matches!(iter.next(), None);
    }

    #[test]
    fn len() {
        let mut ir_code = IrCode::new(&Program::from_string("+++>+"));

        assert_eq!(ir_code.len(), 5);
        ir_code.optimize();
        assert_eq!(ir_code.len(), 3);
    }

    #[test]
    fn optimizes_tail_instructions() {
        let mut ir_code = IrCode::new(&Program::from_string("+++"));
        ir_code.optimize();
        let mut iter = ir_code.iter();

        assert_matches!(iter.next(), Some(IrOp::Add(_, 3)));
        assert_matches!(iter.next(), None);
    }

    #[test]
    fn optimizes_consecutive_adds() {
        let mut ir_code = IrCode::new(&Program::from_string("+++>++"));
        ir_code.optimize();
        let mut iter = ir_code.iter();

        assert_matches!(iter.next(), Some(IrOp::Add(_, 3)));
        assert_matches!(iter.next(), Some(IrOp::Right(_, 1)));
        assert_matches!(iter.next(), Some(IrOp::Add(_, 2)));
        assert_matches!(iter.next(), None);
    }

    #[test]
    fn optimizes_consecutive_subtractions() {
        let mut ir_code = IrCode::new(&Program::from_string("--->-"));
        ir_code.optimize();
        let mut iter = ir_code.iter();

        assert_matches!(iter.next(), Some(IrOp::Sub(_, 3)));
        assert_matches!(iter.next(), Some(IrOp::Right(_, 1)));
        assert_matches!(iter.next(), Some(IrOp::Sub(_, 1)));
        assert_matches!(iter.next(), None);
    }

    #[test]
    fn optimizes_consecutive_lefts_rights() {
        let mut ir_code = IrCode::new(&Program::from_string(">>+>>>-<<<<+"));

        ir_code.optimize();
        let mut iter = ir_code.iter();

        assert_matches!(iter.next(), Some(IrOp::Right(_, 2)));
        assert_matches!(iter.next(), Some(IrOp::Add(_, 1)));
        assert_matches!(iter.next(), Some(IrOp::Right(_, 3)));
        assert_matches!(iter.next(), Some(IrOp::Sub(_, 1)));
        assert_matches!(iter.next(), Some(IrOp::Left(_, 4)));
        assert_matches!(iter.next(), Some(IrOp::Add(_, 1)));
        assert_matches!(iter.next(), None);
    }

    #[test]
    fn optimizes_clear_loops() {
        let mut ir_code = IrCode::new(&Program::from_string("+++[-]-[+]>"));

        ir_code.optimize();
        let mut iter = ir_code.iter();

        assert_matches!(iter.next(), Some(IrOp::Add(_, 3)));
        assert_matches!(iter.next(), Some(IrOp::SetIndirect(_, 0)));
        assert_matches!(iter.next(), Some(IrOp::Sub(_, 1)));
        assert_matches!(iter.next(), Some(IrOp::SetIndirect(_, 0)));
        assert_matches!(iter.next(), Some(IrOp::Right(_, 1)));
        assert_matches!(iter.next(), None);
    }
}

// read program -> tokenize -> parse to ir -> optimize -> transform to asm -> execute
// read program -> tokenize -> parse to ir -> optimize -> interpret

// # Combine multiple instructions

// Right(x), Right(y) -> Right(x+y)
//// Right(x), Left(y) -> Left(x-y)
//// Left(x), Right(y) -> Left(y-x)
//// Left(x), Left(y) -> Left(x+y)
//
//// Add(x), Add(y) -> Add(x+y)
//// Add(x), Sub(y) -> Sub(x-y)
//// Sub(x), Add(y) -> Sub(y-x)
//// Sub(x), Sub(y) -> Sub(x+y)
//
//// JumpIfZero, Sub(1), JumpIfNotZero -> Set(0)
//// JumpIfZero, Add(1), JumpIfNotZero -> Set(0)
//
//// Set(0), Add(x) = Set(x)
//// Set(x), Add(y) = Set(x+y)
//
//// Set(0), JumpIfZero -> JumpIfZero
//
//// Set(x), Set(y) -> Set(y)
//
//// Add(x), Set(y) -> Set(y)
//// Sub(x), Set(y) -> Set(y)
//
//// Add(x), Read -> Read
//// Sub(x), Read -> Read
//// Set(x), Read -> Read
//
//// Add(0) -> Noop
//// Sub(0) -> Noop
//// Left(0) -> Noop
//// Right(0) -> Noop
//
//// Noop, _ -> _
//// _, Noop -> _


// copy loops ? ([>+<-])
// multiplication loops ? ([->++<])
// scan loops ?
// operation offsets ?
