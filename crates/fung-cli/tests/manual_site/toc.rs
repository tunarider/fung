//! The manual's contents file (`manual/toc.txt`): the parts, in order,
//! and their pages. The sidebar, the breadcrumbs, the pager and the
//! cover's cards all come from it.

/// The whole contents.
#[derive(Debug, PartialEq)]
pub struct Toc {
    pub parts: Vec<Part>,
}

#[derive(Debug, PartialEq)]
pub struct Part {
    pub key: String,
    pub glyph: String,
    /// Chapters carry numbers (`3. …`, `3장`).
    pub numbered: bool,
    pub title: String,
    pub blurb: String,
    /// Who the part is for, on the cover.
    pub who: Option<String>,
    pub entries: Vec<Entry>,
}

#[derive(Debug, PartialEq)]
pub enum Entry {
    /// A heading over the pages that follow.
    Group(String),
    /// A manuscript path (`vm/internals.md`).
    Page(String),
}

/// Parses `toc.txt`: `# comment`, `part KEY GLYPH [numbered] TITLE | BLURB [| WHO]`,
/// `group NAME`, `page PATH`.
pub fn parse(text: &str) -> Result<Toc, String> {
    let mut parts: Vec<Part> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let n = i + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (word, rest) = line.split_once(' ').unwrap_or((line, ""));
        let rest = rest.trim();
        match word {
            "part" => {
                let mut words = rest.splitn(3, ' ');
                let (Some(key), Some(glyph), Some(mut tail)) =
                    (words.next(), words.next(), words.next())
                else {
                    return Err(format!(
                        "toc.txt:{n}: `part KEY GLYPH [numbered] TITLE | BLURB [| WHO]`"
                    ));
                };
                let numbered = match tail.strip_prefix("numbered ") {
                    Some(t) => {
                        tail = t;
                        true
                    }
                    None => false,
                };
                let fields: Vec<&str> = tail.split('|').map(str::trim).collect();
                if !(2..=3).contains(&fields.len()) || fields.iter().any(|f| f.is_empty()) {
                    return Err(format!(
                        "toc.txt:{n}: a part is `TITLE | BLURB [| WHO]` after its glyph"
                    ));
                }
                if parts.iter().any(|p| p.key == key) {
                    return Err(format!("toc.txt:{n}: the part `{key}` twice"));
                }
                parts.push(Part {
                    key: key.to_string(),
                    glyph: glyph.to_string(),
                    numbered,
                    title: fields[0].to_string(),
                    blurb: fields[1].to_string(),
                    who: fields.get(2).map(|w| w.to_string()),
                    entries: Vec::new(),
                });
            }
            "group" | "page" => {
                let Some(part) = parts.last_mut() else {
                    return Err(format!("toc.txt:{n}: `{word}` before any `part`"));
                };
                if rest.is_empty() {
                    return Err(format!("toc.txt:{n}: `{word}` needs a name"));
                }
                if word == "group" {
                    part.entries.push(Entry::Group(rest.to_string()));
                } else {
                    if seen.iter().any(|p| p == rest) {
                        return Err(format!("toc.txt:{n}: the page `{rest}` twice"));
                    }
                    seen.push(rest.to_string());
                    part.entries.push(Entry::Page(rest.to_string()));
                }
            }
            _ => {
                return Err(format!(
                    "toc.txt:{n}: unknown line, want part, group or page"
                ));
            }
        }
    }
    if let Some(p) = parts
        .iter()
        .find(|p| !p.entries.iter().any(|e| matches!(e, Entry::Page(_))))
    {
        return Err(format!("toc.txt: the part `{}` has no page", p.key));
    }
    Ok(Toc { parts })
}

impl Part {
    /// The part's pages in order.
    pub fn pages(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().filter_map(|e| match e {
            Entry::Page(p) => Some(p.as_str()),
            Entry::Group(_) => None,
        })
    }
}

impl Toc {
    /// The part holding `page`, the page's number in it (from 1) and the
    /// group it is under.
    pub fn locate(&self, page: &str) -> Option<(&Part, usize, Option<&str>)> {
        for part in &self.parts {
            let (mut n, mut group) = (0, None);
            for e in &part.entries {
                match e {
                    Entry::Group(g) => group = Some(g.as_str()),
                    Entry::Page(p) => {
                        n += 1;
                        if p == page {
                            return Some((part, n, group));
                        }
                    }
                }
            }
        }
        None
    }
}
