use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// An interned symbol (`'ok`). Equal symbols share one id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Sym(u32);

struct Table {
    ids: HashMap<&'static str, u32>,
    names: Vec<&'static str>,
}

fn table() -> &'static Mutex<Table> {
    static T: OnceLock<Mutex<Table>> = OnceLock::new();
    T.get_or_init(|| {
        Mutex::new(Table {
            ids: HashMap::new(),
            names: Vec::new(),
        })
    })
}

/// Returns the symbol for a name, creating it on first use.
pub fn intern(name: &str) -> Sym {
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

impl Sym {
    /// The symbol's text without the quote.
    pub fn name(self) -> &'static str {
        let t = table().lock().unwrap_or_else(|e| e.into_inner());
        t.names[self.0 as usize]
    }
}
