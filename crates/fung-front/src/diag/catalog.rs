//! The error catalog, parsed once from `spec/errors.txt`.

use std::collections::BTreeMap;
use std::sync::OnceLock;

pub struct Entry {
    pub message: String,
    pub label: String,
    pub help: String,
}

const SPEC: &str = include_str!("../../../../spec/errors.txt");

/// Looks up the catalog entry for a code.
pub fn entry(code: &str) -> Option<&'static Entry> {
    static CATALOG: OnceLock<BTreeMap<String, Entry>> = OnceLock::new();
    CATALOG.get_or_init(|| parse(SPEC)).get(code)
}

fn parse(spec: &str) -> BTreeMap<String, Entry> {
    let mut map = BTreeMap::new();
    for line in spec.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let mut cols = line.splitn(4, '\t');
        let code = cols.next().unwrap_or("");
        let message = cols.next().unwrap_or("");
        let label = cols.next().unwrap_or("");
        let help = cols.next().unwrap_or("");
        map.insert(
            code.to_string(),
            Entry {
                message: message.to_string(),
                label: label.to_string(),
                help: help.to_string(),
            },
        );
    }
    map
}
