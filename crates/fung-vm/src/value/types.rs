use super::{Value, type_name};

/// Whether `v` satisfies a union of type names: `any` always, `num` for int
/// or float, otherwise the type name (a tag name for a tagged value).
/// `None` means unchecked.
pub fn matches_type(v: &Value, alts: Option<&[String]>) -> bool {
    let Some(alts) = alts else { return true };
    let tn = type_name(v);
    alts.iter().any(|a| match a.as_str() {
        "any" => true,
        "num" => tn == "int" || tn == "float",
        other => other == tn,
    })
}
