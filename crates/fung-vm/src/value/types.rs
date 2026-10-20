use super::{Sym, Value, intern};

/// A union of type names made ready to check without allocating (07; P6):
/// the value kinds it takes as bits, and the tag names it takes. A tag
/// name takes a tagged value of that name and the symbol of that name,
/// which is the tag without values (T1).
#[derive(Debug)]
pub struct TypeSet {
    kinds: u32,
    tags: Vec<Sym>,
    /// The union as written, for errors (`int | nil`).
    text: String,
}

const INT: u32 = 1 << 0;
const FLOAT: u32 = 1 << 1;
const SYM: u32 = 1 << 5;
const ALL: u32 = u32::MAX;

/// The kind bit of each type name `type_name` gives (a tag aside).
fn kind_of_name(name: &str) -> Option<u32> {
    let bit = |i: u32| Some(1 << i);
    match name {
        "int" => Some(INT),
        "float" => Some(FLOAT),
        "bool" => bit(2),
        "nil" => bit(3),
        "str" => bit(4),
        "sym" => Some(SYM),
        "dur" => bit(6),
        "time" => bit(7),
        "list" => bit(8),
        "map" => bit(9),
        "tuple" => bit(10),
        "range" => bit(11),
        "fn" => bit(12),
        "module" => bit(13),
        "task" => bit(14),
        "scope" => bit(15),
        "atom" => bit(16),
        "sender" => bit(17),
        "receiver" => bit(18),
        "file" => bit(19),
        "conn" => bit(20),
        "listener" => bit(21),
        _ => None,
    }
}

/// The kind bit of a value; 0 for a tagged value (its name decides).
fn kind_of(v: &Value) -> u32 {
    let bit = |i: u32| 1 << i;
    match v {
        Value::Int(_) => INT,
        Value::Float(_) => FLOAT,
        Value::Bool(_) => bit(2),
        Value::Nil => bit(3),
        Value::Str(_) => bit(4),
        Value::Sym(_) => SYM,
        Value::Dur(_) => bit(6),
        Value::Time(_) => bit(7),
        Value::List(_) => bit(8),
        Value::Map(_) => bit(9),
        Value::Tuple(_) => bit(10),
        Value::Range(_) => bit(11),
        Value::Closure(_) | Value::Native(_) => bit(12),
        Value::Module(_) => bit(13),
        Value::Task(_) => bit(14),
        Value::Scope(_) => bit(15),
        Value::Atom(_) => bit(16),
        Value::Sender(_) => bit(17),
        Value::Receiver(_) => bit(18),
        Value::File(_) => bit(19),
        Value::Conn(_) => bit(20),
        Value::Listener(_) => bit(21),
        Value::Tag(_) => 0,
    }
}

impl TypeSet {
    /// `any` takes everything, `num` an int or a float; a name that starts
    /// with an uppercase letter is a tag (01). Any other name matches
    /// nothing, as no value has it.
    pub fn new(alts: &[String]) -> TypeSet {
        let mut t = TypeSet {
            kinds: 0,
            tags: Vec::new(),
            text: alts.join(" | "),
        };
        for a in alts {
            match a.as_str() {
                "any" => t.kinds = ALL,
                "num" => t.kinds |= INT | FLOAT,
                name if name.chars().next().is_some_and(char::is_uppercase) => {
                    t.tags.push(intern(name));
                }
                name => t.kinds |= kind_of_name(name).unwrap_or(0),
            }
        }
        t
    }

    pub fn matches(&self, v: &Value) -> bool {
        match v {
            Value::Tag(tag) => self.kinds == ALL || self.tags.contains(&tag.name),
            Value::Sym(s) => self.kinds & SYM != 0 || self.tags.contains(s),
            _ => self.kinds & kind_of(v) != 0,
        }
    }

    /// The union as written.
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// Whether `v` satisfies a union of type names; `None` means unchecked.
pub fn matches_type(v: &Value, alts: Option<&[String]>) -> bool {
    alts.is_none_or(|a| TypeSet::new(a).matches(v))
}
