//! Opcodes and the 32-bit instruction word (09. 명령 인코딩): 8 bits of
//! opcode and 24 bits of operand.

/// One VM operation. The numbers are fixed: `fung dump bytecode` and its
/// expected outputs show them.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Op {
    /// push consts[arg]
    Const = 0,
    /// discard top
    Pop,
    Add,
    Sub,
    Mul,
    /// always float
    Div,
    /// `//`
    FloorDiv,
    Mod,
    Pow,
    Neg,
    /// a must be bool
    Not,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    /// error unless the top is bool
    CheckBool,
    Jump,
    JumpIfFalse,
    /// arg = encode_call(positional, named)
    Call,
    Return,
    LoadLocal,
    StoreLocal,
    /// E0207 if unassigned
    LoadGlobal,
    StoreGlobal,
    /// arg = part count; parts formatted as print would
    Concat,
    /// arg = index into protos
    Closure,
    LoadCaptured,
    LoadSelf,
    /// like CALL, reusing the current frame
    TailCall,
    /// if the slot value is assigned, ip = arg
    JumpIfSet,
    /// slot `arg & 0xFFF` must match `types[arg >> 12]` (E0403)
    CheckType,
    /// the top must match `types[arg]` (E0404)
    CheckReturn,
    List,
    /// keys are Str
    Map,
    Tuple,
    /// name a1..an -- tag; the name is a Sym
    Tag,
    /// arg = 1 if inclusive
    Range,
    Index,
    /// arg bit0 = has start, bit1 = has end, bit2 = inclusive
    Slice,
    /// arg = const index of the Str name
    Field,
    In,
    /// `!`
    UnwrapAbort,
    /// `?`
    UnwrapTest,
    /// `??`
    Coalesce,
    /// arg = patterns index; binds on success
    Match,
    /// E0602 on failure
    MatchOrFail,
    /// E0601 with the value
    NoMatch,
    /// arg = const index of the Str module key
    Import,
    /// arg = asserts index; E0807 on failure
    Assert,
    /// arg = const index of the Str test name
    Test,
}

impl Op {
    /// Every opcode in numbering order.
    pub const ALL: [Op; 52] = [
        Op::Const,
        Op::Pop,
        Op::Add,
        Op::Sub,
        Op::Mul,
        Op::Div,
        Op::FloorDiv,
        Op::Mod,
        Op::Pow,
        Op::Neg,
        Op::Not,
        Op::Eq,
        Op::Ne,
        Op::Lt,
        Op::Gt,
        Op::Le,
        Op::Ge,
        Op::CheckBool,
        Op::Jump,
        Op::JumpIfFalse,
        Op::Call,
        Op::Return,
        Op::LoadLocal,
        Op::StoreLocal,
        Op::LoadGlobal,
        Op::StoreGlobal,
        Op::Concat,
        Op::Closure,
        Op::LoadCaptured,
        Op::LoadSelf,
        Op::TailCall,
        Op::JumpIfSet,
        Op::CheckType,
        Op::CheckReturn,
        Op::List,
        Op::Map,
        Op::Tuple,
        Op::Tag,
        Op::Range,
        Op::Index,
        Op::Slice,
        Op::Field,
        Op::In,
        Op::UnwrapAbort,
        Op::UnwrapTest,
        Op::Coalesce,
        Op::Match,
        Op::MatchOrFail,
        Op::NoMatch,
        Op::Import,
        Op::Assert,
        Op::Test,
    ];

    pub fn from_u8(b: u8) -> Option<Op> {
        Op::ALL.get(b as usize).copied()
    }

    /// The name the disassembler prints.
    pub fn name(self) -> &'static str {
        match self {
            Op::Const => "CONST",
            Op::Pop => "POP",
            Op::Add => "ADD",
            Op::Sub => "SUB",
            Op::Mul => "MUL",
            Op::Div => "DIV",
            Op::FloorDiv => "FLOORDIV",
            Op::Mod => "MOD",
            Op::Pow => "POW",
            Op::Neg => "NEG",
            Op::Not => "NOT",
            Op::Eq => "EQ",
            Op::Ne => "NE",
            Op::Lt => "LT",
            Op::Gt => "GT",
            Op::Le => "LE",
            Op::Ge => "GE",
            Op::CheckBool => "CHECKBOOL",
            Op::Jump => "JUMP",
            Op::JumpIfFalse => "JUMPIFFALSE",
            Op::Call => "CALL",
            Op::Return => "RETURN",
            Op::LoadLocal => "LOADLOCAL",
            Op::StoreLocal => "STORELOCAL",
            Op::LoadGlobal => "LOADGLOBAL",
            Op::StoreGlobal => "STOREGLOBAL",
            Op::Concat => "CONCAT",
            Op::Closure => "CLOSURE",
            Op::LoadCaptured => "LOADCAPTURED",
            Op::LoadSelf => "LOADSELF",
            Op::TailCall => "TAILCALL",
            Op::JumpIfSet => "JUMPIFSET",
            Op::CheckType => "CHECKTYPE",
            Op::CheckReturn => "CHECKRETURN",
            Op::List => "LIST",
            Op::Map => "MAP",
            Op::Tuple => "TUPLE",
            Op::Tag => "TAG",
            Op::Range => "RANGE",
            Op::Index => "INDEX",
            Op::Slice => "SLICE",
            Op::Field => "FIELD",
            Op::In => "IN",
            Op::UnwrapAbort => "UNWRAPABORT",
            Op::UnwrapTest => "UNWRAPTEST",
            Op::Coalesce => "COALESCE",
            Op::Match => "MATCH",
            Op::MatchOrFail => "MATCHORFAIL",
            Op::NoMatch => "NOMATCH",
            Op::Import => "IMPORT",
            Op::Assert => "ASSERT",
            Op::Test => "TEST",
        }
    }

    /// Whether the operand is meaningful for this opcode.
    pub fn has_arg(self) -> bool {
        !matches!(
            self,
            Op::Pop
                | Op::Add
                | Op::Sub
                | Op::Mul
                | Op::Div
                | Op::FloorDiv
                | Op::Mod
                | Op::Pow
                | Op::Neg
                | Op::Not
                | Op::Eq
                | Op::Ne
                | Op::Lt
                | Op::Gt
                | Op::Le
                | Op::Ge
                | Op::CheckBool
                | Op::Return
                | Op::LoadSelf
                | Op::Index
                | Op::In
                | Op::UnwrapAbort
                | Op::NoMatch
        )
    }
}

const ARG_MASK: u32 = (1 << 24) - 1;

/// One instruction word: opcode in the top 8 bits, operand below.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Instr(pub u32);

impl Instr {
    /// Encodes an instruction; the operand is truncated to 24 bits.
    pub fn new(op: Op, arg: usize) -> Instr {
        Instr((op as u32) << 24 | (arg as u32) & ARG_MASK)
    }

    pub fn op(self) -> Op {
        Op::from_u8((self.0 >> 24) as u8).expect("instruction with a valid opcode")
    }

    pub fn arg(self) -> usize {
        (self.0 & ARG_MASK) as usize
    }
}

/// CALL and TAILCALL pack the positional count in the low 12 bits of the
/// operand and the named-argument count in the next 12.
pub fn encode_call(positional: usize, named: usize) -> usize {
    positional | named << 12
}

/// Unpacks a CALL operand into (positional, named).
pub fn decode_call(arg: usize) -> (usize, usize) {
    (arg & 0xFFF, arg >> 12)
}
