use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// An interned symbol (`'ok`). Equal symbols share one id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Sym(u32);

struct Table {
    ids: HashMap<&'static str, u32>,
    names: Vec<&'static str>,
}

/// The names the table starts with, at fixed ids: the run loop compares
/// a tag with `Ok` and `Err` by number, without the table (VM7).
const RESERVED: [&str; 2] = ["Ok", "Err"];

fn table() -> &'static Mutex<Table> {
    static T: OnceLock<Mutex<Table>> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = Table {
            ids: HashMap::new(),
            names: Vec::new(),
        };
        for name in RESERVED {
            t.ids.insert(name, t.names.len() as u32);
            t.names.push(name);
        }
        Mutex::new(t)
    })
}

/// Returns the symbol for a name, creating it on first use.
pub fn intern(name: &str) -> Sym {
    match name {
        "Ok" => return Sym::OK,
        "Err" => return Sym::ERR,
        _ => {}
    }
    let mut t = table().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(&id) = t.ids.get(name) {
        return Sym(id);
    }
    // Symbol names live for the whole process.
    let leaked: &'static str = Box::leak(name.to_string().into_boxed_str());
    let id = t.names.len() as u32;
    t.names.push(leaked);
    t.ids.insert(leaked, id);
    Sym(id)
}

thread_local! {
    /// This thread's copy of the names: the table only grows, so a copy
    /// is right for every id it holds, and a name is read without the
    /// lock (VM7). An id past its end refreshes it.
    static NAMES: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}

impl Sym {
    /// `Ok`, the tag of a successful result.
    pub const OK: Sym = Sym(0);
    /// `Err`, the tag of a failed result.
    pub const ERR: Sym = Sym(1);

    /// The symbol's text without the quote.
    pub fn name(self) -> &'static str {
        let i = self.0 as usize;
        if let Some(n) = NAMES.with(|c| c.borrow().get(i).copied()) {
            return n;
        }
        let t = table().lock().unwrap_or_else(|e| e.into_inner());
        let name = t.names[i];
        NAMES.with(|c| {
            let mut c = c.borrow_mut();
            let from = c.len();
            c.extend_from_slice(&t.names[from..]);
        });
        name
    }
}

#[cfg(test)]
mod tests {
    use super::{Sym, intern};

    /// `Ok` and `Err` are the reserved ids, and names read back on any
    /// thread, also ones interned after the thread first read (VM7).
    #[test]
    fn reserved_ids_and_names_on_every_thread() {
        assert_eq!(intern("Ok"), Sym::OK);
        assert_eq!(intern("Err"), Sym::ERR);
        assert_eq!((Sym::OK.name(), Sym::ERR.name()), ("Ok", "Err"));
        let a = intern("sym_test_a");
        assert_eq!(a.name(), "sym_test_a");
        let b = intern("sym_test_b");
        assert_eq!(b.name(), "sym_test_b");
        let c = std::thread::spawn(|| intern("sym_test_c")).join().unwrap();
        assert_eq!(c.name(), "sym_test_c");
        assert_eq!(
            std::thread::spawn(move || a.name()).join().unwrap(),
            "sym_test_a"
        );
    }
}
