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
    /* offset, factor */
    MulCopy(Link, u8, u8),
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
            IrOp::MulCopy(l, _, _) => l,
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
            (IrOp::Add(_, x), IrOp::Add(far, y)) => IrOp::Add(*far, *x + *y),
            (IrOp::Sub(_, x), IrOp::Sub(far, y)) => IrOp::Sub(*far, *x + *y),
            (IrOp::Sub(_, x), IrOp::Add(far, y)) => {
                let result = *y as i8 - *x as i8;
                if result > 0 { IrOp::Add(*far, result as u8) } else { IrOp::Sub(*far, -result as u8) }
            }
            (IrOp::Add(_, x), IrOp::Sub(far, y)) => {
                let result = *x as i8 - *y as i8;
                if result > 0 { IrOp::Add(*far, result as u8) } else { IrOp::Sub(*far, -result as u8) }
            }

            (IrOp::Right(_, x), IrOp::Right(far, y)) => IrOp::Right(*far, *x + *y),
            (IrOp::Left(_, x), IrOp::Left(far, y)) => IrOp::Left(*far, *x + *y),
            (IrOp::Right(_, x), IrOp::Left(far, y)) => {
                let result = *x as i8 - *y as i8;
                if result > 0 { IrOp::Right(*far, result as u8) } else { IrOp::Left(*far, -result as u8) }
            }
            (IrOp::Left(_, x), IrOp::Right(far, y)) => {
                let result = *y as i8 - *x as i8;
                if result > 0 { IrOp::Right(*far, result as u8) } else { IrOp::Left(*far, -result as u8) }
            }

            (IrOp::SetIndirect(_, c), IrOp::Add(far, x)) => IrOp::SetIndirect(*far, c + x),
            (IrOp::SetIndirect(_, c), IrOp::Sub(far, x)) => IrOp::SetIndirect(*far, c.wrapping_sub(*x)),

            (IrOp::Add(_, _), IrOp::SetIndirect(far, c)) => IrOp::SetIndirect(*far, *c),
            (IrOp::Sub(_, _), IrOp::SetIndirect(far, c)) => IrOp::SetIndirect(*far, *c),

            (IrOp::SetIndirect(_, _), IrOp::SetIndirect(far, c)) => IrOp::SetIndirect(*far, *c),

            (c, _) => *c,
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
        let ir_code = IrCode::new(&Program::from_string("+-<>.,"));
        let mut iter = ir_code.iter();

        assert_matches!(iter.next(), Some(IrOp::Add(_, 1)));
        assert_matches!(iter.next(), Some(IrOp::Sub(_, 1)));
        assert_matches!(iter.next(), Some(IrOp::Left(_, 1)));
        assert_matches!(iter.next(), Some(IrOp::Right(_, 1)));
        assert_matches!(iter.next(), Some(IrOp::Write(_)));
        assert_matches!(iter.next(), Some(IrOp::Read(_)));
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
    fn optimizes_consecutive_mixed_adds() {
        let mut ir_code = IrCode::new(&Program::from_string("+++-->---++>--+++>++---"));
        ir_code.optimize();
        let mut iter = ir_code.iter();

        assert_matches!(iter.next(), Some(IrOp::Add(_, 1)));
        assert_matches!(iter.next(), Some(IrOp::Right(_, 1)));
        assert_matches!(iter.next(), Some(IrOp::Sub(_, 1)));
        assert_matches!(iter.next(), Some(IrOp::Right(_, 1)));
        assert_matches!(iter.next(), Some(IrOp::Add(_, 1)));
        assert_matches!(iter.next(), Some(IrOp::Right(_, 1)));
        assert_matches!(iter.next(), Some(IrOp::Sub(_, 1)));
        assert_matches!(iter.next(), None);
    }

    #[test]
    fn optimizes_consecutive_mixed_lefts_rights() {
        let mut ir_code = IrCode::new(&Program::from_string(">>><<+<<<>>+<<>>>+>><<<"));
        ir_code.optimize();
        let mut iter = ir_code.iter();

        assert_matches!(iter.next(), Some(IrOp::Right(_, 1)));
        assert_matches!(iter.next(), Some(IrOp::Add(_, 1)));
        assert_matches!(iter.next(), Some(IrOp::Left(_, 1)));
        assert_matches!(iter.next(), Some(IrOp::Add(_, 1)));
        assert_matches!(iter.next(), Some(IrOp::Right(_, 1)));
        assert_matches!(iter.next(), Some(IrOp::Add(_, 1)));
        assert_matches!(iter.next(), Some(IrOp::Left(_, 1)));
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
        let mut ir_code = IrCode::new(&Program::from_string("[-]>[+]>"));

        ir_code.optimize();
        let mut iter = ir_code.iter();

        assert_matches!(iter.next(), Some(IrOp::SetIndirect(_, 0)));
        assert_matches!(iter.next(), Some(IrOp::Right(_, 1)));
        assert_matches!(iter.next(), Some(IrOp::SetIndirect(_, 0)));
        assert_matches!(iter.next(), Some(IrOp::Right(_, 1)));
        assert_matches!(iter.next(), None);
    }

    #[test]
    fn optimizes_adds_following_preceding_clear_loops() {
        let mut ir_code = IrCode::new(&Program::from_string("+[-]+++++>-[+]----"));

        ir_code.optimize();
        let mut iter = ir_code.iter();

        assert_matches!(iter.next(), Some(IrOp::SetIndirect(_, 5)));
        assert_matches!(iter.next(), Some(IrOp::Right(_, 1)));
        assert_matches!(iter.next(), Some(IrOp::SetIndirect(_, 252)));
        assert_matches!(iter.next(), None);
    }

    #[test]
    fn optimizes_consecutive_sets() {
        let mut ir_code = IrCode::new(&Program::from_string("+[-]+++++-[+]----"));

        ir_code.optimize();
        let mut iter = ir_code.iter();

        assert_eq!(ir_code.len(), 1);

        assert_matches!(iter.next(), Some(IrOp::SetIndirect(_, 252)));
        assert_matches!(iter.next(), None);
    }
}