//! Opcodes and the instruction word (09. 명령 인코딩): 8 bits of opcode,
//! a 24-bit operand, and a second 24-bit operand for the few
//! instructions that need two (VM32: MATCH, TAG, MAP; VM42, VM43: the
//! calls, CHECKTYPE, JUMPIFSET).
//!
//! One table, `instructions!` below, says everything the front end knows
//! about an opcode: its name, what its operand indexes, how it changes
//! the stack and where control goes after it (VM33). The disassembler,
//! the operand-limit error and the verifier (`verify`) all read it, so a
//! new instruction is one line here and its case in the VM.

/// What an instruction's operand means: the table it indexes, a count, or
/// nothing. The verifier checks it is in range.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Operand {
    /// No operand; 0 is emitted.
    None,
    /// An index into `Proto::consts`.
    Const,
    /// An index into the file's globals (the main proto's `global_names`).
    Global,
    /// A local slot, below `Proto::num_locals`.
    Local,
    /// An index into `Proto::protos`.
    Proto,
    /// An index into `Proto::captures`.
    Captured,
    /// An index into `Proto::types`.
    Type,
    /// An index into `Proto::patterns`.
    Pattern,
    /// An index into `Proto::asserts`.
    Assert,
    /// An instruction index in the same proto: a jump target.
    Target,
    /// How many values the instruction takes from the stack (or pairs).
    Count,
    /// Bit flags (SLICE) or 1 for an inclusive RANGE.
    Flags,
}

/// How an instruction changes the operand stack on the path that goes on
/// to the next instruction: how many values it takes and how many it
/// leaves. Most are fixed; the rest depend on the operand.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Effect {
    /// Takes the first, leaves the second.
    Fixed(u8, u8),
    /// Takes `arg` values, leaves one (LIST, TUPLE, CONCAT, MAP, TAG).
    Items,
    /// Takes the value and the parts its flags name, leaves one (SLICE).
    Sliced,
    /// Takes the callee, the positional arguments and a (symbol, value)
    /// pair per named argument, leaves the result (CALL, TAILCALL).
    Called,
    /// Takes the condition, or both sides of a split comparison (ASSERT).
    Asserted,
}

/// Where control goes after an instruction.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flow {
    /// To the next instruction.
    Next,
    /// To the operand only (JUMP).
    Goto,
    /// To the next instruction or the operand, with the same stack.
    Branch,
    /// To the next instruction, or to the operand with one value more
    /// than the next instruction sees (COALESCE keeps a value it jumps
    /// with and drops nil or an Err).
    BranchKeep,
    /// Nowhere: the frame ends or fails here.
    Stop,
}

/// The facts the table holds about one opcode.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Info {
    pub operand: Operand,
    pub effect: Effect,
    pub flow: Flow,
}

macro_rules! instructions {
    ($(
        $(#[doc = $doc:literal])*
        $op:ident $name:literal $operand:ident $effect:expr, $flow:ident;
    )*) => {
        /// One VM operation. Its number is its place in the table; the VM
        /// and the front end are built together, so nothing outside the
        /// process sees it (`fung dump bytecode` shows names).
        #[repr(u8)]
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub enum Op {
            $( $(#[doc = $doc])* $op, )*
        }

        impl Op {
            /// Every opcode in numbering order.
            pub const ALL: [Op; [$(stringify!($op)),*].len()] = [$(Op::$op),*];

            /// The name the disassembler prints.
            pub fn name(self) -> &'static str {
                match self {
                    $(Op::$op => $name,)*
                }
            }

            /// What the table says about this opcode.
            pub fn info(self) -> Info {
                use Effect::*;
                match self {
                    $(Op::$op => Info {
                        operand: Operand::$operand,
                        effect: $effect,
                        flow: Flow::$flow,
                    },)*
                }
            }
        }
    };
}

instructions! {
    /// push consts[arg]
    Const "CONST" Const Fixed(0, 1), Next;
    /// discard top
    Pop "POP" None Fixed(1, 0), Next;
    Add "ADD" None Fixed(2, 1), Next;
    Sub "SUB" None Fixed(2, 1), Next;
    Mul "MUL" None Fixed(2, 1), Next;
    /// always float
    Div "DIV" None Fixed(2, 1), Next;
    /// `//`
    FloorDiv "FLOORDIV" None Fixed(2, 1), Next;
    Mod "MOD" None Fixed(2, 1), Next;
    Pow "POW" None Fixed(2, 1), Next;
    Neg "NEG" None Fixed(1, 1), Next;
    /// a must be bool
    Not "NOT" None Fixed(1, 1), Next;
    Eq "EQ" None Fixed(2, 1), Next;
    Ne "NE" None Fixed(2, 1), Next;
    Lt "LT" None Fixed(2, 1), Next;
    Gt "GT" None Fixed(2, 1), Next;
    Le "LE" None Fixed(2, 1), Next;
    Ge "GE" None Fixed(2, 1), Next;
    /// error unless the top is bool
    CheckBool "CHECKBOOL" None Fixed(1, 1), Next;
    Jump "JUMP" Target Fixed(0, 0), Goto;
    JumpIfFalse "JUMPIFFALSE" Target Fixed(1, 0), Branch;
    /// arg = positional arguments, b = named arguments
    Call "CALL" Count Called, Next;
    Return "RETURN" None Fixed(1, 0), Stop;
    LoadLocal "LOADLOCAL" Local Fixed(0, 1), Next;
    StoreLocal "STORELOCAL" Local Fixed(1, 0), Next;
    /// unassigned: E0808 in the REPL, E0809 otherwise (a constant read early)
    LoadGlobal "LOADGLOBAL" Global Fixed(0, 1), Next;
    StoreGlobal "STOREGLOBAL" Global Fixed(1, 0), Next;
    /// arg = part count; parts formatted as print would
    Concat "CONCAT" Count Items, Next;
    /// arg = index into protos
    Closure "CLOSURE" Proto Fixed(0, 1), Next;
    LoadCaptured "LOADCAPTURED" Captured Fixed(0, 1), Next;
    LoadSelf "LOADSELF" None Fixed(0, 1), Next;
    /// like CALL, reusing the current frame
    TailCall "TAILCALL" Count Called, Stop;
    /// if the parameter slot b is assigned, ip = arg (a default's test)
    JumpIfSet "JUMPIFSET" Target Fixed(0, 0), Branch;
    /// the parameter slot arg must match `types[b]` (E0403)
    CheckType "CHECKTYPE" Local Fixed(0, 0), Next;
    /// the top must match `types[arg]` (E0404)
    CheckReturn "CHECKRETURN" Type Fixed(1, 1), Next;
    List "LIST" Count Items, Next;
    /// arg values; b = const index of the list of their Str keys
    Map "MAP" Count Items, Next;
    Tuple "TUPLE" Count Items, Next;
    /// a1..an -- tag; b = const index of the Sym name
    Tag "TAG" Count Items, Next;
    /// arg = 1 if inclusive
    Range "RANGE" Flags Fixed(2, 1), Next;
    Index "INDEX" None Fixed(2, 1), Next;
    /// arg bit0 = has start, bit1 = has end, bit2 = inclusive
    Slice "SLICE" Flags Sliced, Next;
    /// arg = const index of the Str name
    Field "FIELD" Const Fixed(1, 1), Next;
    In "IN" None Fixed(2, 1), Next;
    /// `!`
    UnwrapAbort "UNWRAPABORT" None Fixed(1, 1), Next;
    /// `?`: an Err stays on the stack and goes to arg (a RETURN)
    UnwrapTest "UNWRAPTEST" Target Fixed(1, 1), Branch;
    /// `??`: any other value goes to arg (an Ok as its payload); nil or
    /// an Err is dropped and the right side follows (VM26)
    Coalesce "COALESCE" Target Fixed(1, 0), BranchKeep;
    /// b = patterns index; binds and goes on on success, else ip = arg
    Match "MATCH" Target Fixed(1, 0), Branch;
    /// E0602 on failure
    MatchOrFail "MATCHORFAIL" Pattern Fixed(1, 0), Next;
    /// E0601 with the value
    NoMatch "NOMATCH" None Fixed(1, 0), Stop;
    /// arg = const index of the Str module key
    Import "IMPORT" Const Fixed(0, 1), Next;
    /// arg = asserts index; E0807 on failure
    Assert "ASSERT" Assert Asserted, Next;
    /// arg = const index of the Str test name
    Test "TEST" Const Fixed(1, 0), Next;
    /// let go of slot arg: it is not read again (03: 값의 수명)
    ClearLocal "CLEARLOCAL" Local Fixed(0, 0), Next;
    /// push local slot arg and empty the slot: its last read (03: 값의
    /// 수명), which hands the value over
    MoveLocal "MOVELOCAL" Local Fixed(0, 1), Next;
    /// like CALL; the last positional argument is a list or a range
    /// whose items are the arguments there (`f(a, ...xs)`, D45)
    CallSpread "CALLSPREAD" Count Called, Next;
    /// like TAILCALL, spreading as CALLSPREAD does
    TailCallSpread "TAILCALLSPREAD" Count Called, Stop;
    /// before a TAILCALL in a function with a return type: the frame's
    /// value is checked against `types[arg]` when it returns (E0404),
    /// whatever function the frame runs by then (D44)
    TailCheck "TAILCHECK" Type Fixed(0, 0), Next;
    /// jump when the top is true; error unless it is a bool (VM27)
    JumpIfTrue "JUMPIFTRUE" Target Fixed(1, 0), Branch;
    /// the frame takes a bool check along into the tail call that
    /// follows: the right side of `and`/`or` in tail position (D97)
    TailCheckBool "TAILCHECKBOOL" None Fixed(0, 0), Next;
}

impl Op {
    /// What the second operand means (VM32): MATCH's pattern (its first
    /// is where it goes when the pattern fails, VM29), TAG's name and
    /// MAP's key list (constants, VM30), a call's named-argument count
    /// (VM43), CHECKTYPE's type and JUMPIFSET's parameter slot (VM42);
    /// `None` for the others.
    pub fn second(self) -> Operand {
        match self {
            Op::Match => Operand::Pattern,
            Op::Tag | Op::Map => Operand::Const,
            Op::Call | Op::TailCall | Op::CallSpread | Op::TailCallSpread => Operand::Count,
            Op::CheckType => Operand::Type,
            Op::JumpIfSet => Operand::Local,
            _ => Operand::None,
        }
    }

    pub fn from_u8(b: u8) -> Option<Op> {
        Op::ALL.get(b as usize).copied()
    }

    /// Whether the operand is meaningful for this opcode.
    pub fn has_arg(self) -> bool {
        self.info().operand != Operand::None
    }

    /// The jump target in the operand, for an instruction that may jump.
    pub fn jumps(self) -> bool {
        self.info().operand == Operand::Target
    }

    /// How many values the instruction takes from the stack and leaves on
    /// it on the path to the next instruction (`Flow`). `b` is the second
    /// operand. ASSERT looks at its entry in `asserts`: `split` says it
    /// compares two sides.
    pub fn stack_effect(self, arg: usize, b: usize, split: bool) -> (usize, usize) {
        match self.info().effect {
            Effect::Fixed(pops, pushes) => (pops as usize, pushes as usize),
            Effect::Items => (arg, 1),
            Effect::Sliced => (1 + (arg & 1) + (arg >> 1 & 1), 1),
            Effect::Called => (1 + arg + 2 * b, 1),
            Effect::Asserted => (1 + usize::from(split), 0),
        }
    }
}

/// The largest operand an instruction holds: 24 bits (09). The compiler
/// refuses a larger one (E0230, `Proto::check_operands`).
pub const MAX_OPERAND: usize = (1 << 24) - 1;

const ARG_MASK: u32 = MAX_OPERAND as u32;

/// One instruction word: the opcode in bits 24-31, the operand below it,
/// the second operand (`Op::second`) in bits 32-55.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Instr(pub u64);

impl Instr {
    /// Encodes an instruction; the operand is cut to 24 bits (`emit`
    /// remembers one that does not fit, `Proto::check_operands`).
    pub fn new(op: Op, arg: usize) -> Instr {
        Instr::with(op, arg, 0)
    }

    /// An instruction with a second operand, cut to 24 bits as the first.
    pub fn with(op: Op, arg: usize, b: usize) -> Instr {
        Instr(
            u64::from((op as u32) << 24 | (arg as u32) & ARG_MASK)
                | u64::from((b as u32) & ARG_MASK) << 32,
        )
    }

    pub fn op(self) -> Op {
        Op::from_u8((self.0 >> 24 & 0xFF) as u8).expect("instruction with a valid opcode")
    }

    pub fn arg(self) -> usize {
        (self.0 as u32 & ARG_MASK) as usize
    }

    /// The second operand (`Op::second`), 0 for the others.
    pub fn b(self) -> usize {
        (self.0 >> 32) as usize & MAX_OPERAND
    }
}
