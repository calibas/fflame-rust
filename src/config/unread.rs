//! What a config held that this build could not read, kept so that a save
//! gives it back (docs/projects/api-updates.md, rule 5).
//!
//! Two kinds of thing, both found at load and both put back by
//! [`Unread::restore`] when the config is written:
//!
//! - **A value that failed to read**: an enum value a newer app added (a
//!   colour mode, a tone map mode), a field whose type it changed. The
//!   load is retried without it ([`deserialize_lenient`]): its field takes
//!   its default, the rest of the config loads. On save the value goes
//!   back, unless the user has since changed that field.
//! - **A field this build does not know**, in a config a newer app wrote
//!   ([`Unread::capture_unknown_fields`]): kept as it was and written back.
//!   Only for a newer writer's config: in an older one every field is
//!   known by definition, and one that is not was retired on purpose (an
//!   alias, a dead key) and must not come back.
//!
//! Places are JSON pointers into the config's JSON (`/escape/coloring`),
//! so this works through every struct, custom serde or not, without a
//! field of its own on any of them.

use serde::de::DeserializeOwned;
use serde_json::Value;

/// One thing set aside at load.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Where it was, as a JSON pointer.
    pub pointer: String,
    /// What was there.
    pub value: Value,
    /// What the config wrote at that place right after loading (the
    /// field's default), `None` when it wrote nothing there. A save that
    /// finds the same puts `value` back; anything else is the user's
    /// edit, which wins.
    pub filled: Option<Value>,
}

/// Everything a config could not read, and the versions it came with.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Unread {
    pub entries: Vec<Entry>,
    /// The config's `min_reader` when it was read, if higher than this
    /// build's own: a save keeps it, so a reader too old for what is kept
    /// here still refuses the file.
    pub min_reader: Option<u32>,
    /// The version that wrote the config, when newer than this build.
    pub newer_version: Option<u32>,
}

impl Unread {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Record the fields of `input` (the config's JSON as read) that
    /// `output` (what the loaded config writes, before defaults are
    /// stripped) does not have: fields this build does not know.
    pub fn capture_unknown_fields(&mut self, input: &Value, output: &Value) {
        let mut pointer = String::new();
        diff(input, output, &mut pointer, &mut self.entries);
    }

    /// Put everything back into `out`, a config's JSON being written:
    /// each entry whose place still holds what loading filled there (or
    /// nothing). Entries go back in reverse order, undoing the removals
    /// that made them.
    pub fn restore(&self, out: &mut Value) {
        for e in self.entries.iter().rev() {
            let segments: Vec<String> = split(&e.pointer);
            let Some((last, parents)) = segments.split_last() else { continue };
            let Some(parent) = walk_creating(out, parents) else { continue };
            match parent {
                Value::Object(map) => {
                    if map.get(last.as_str()) == e.filled.as_ref() {
                        map.insert(last.clone(), e.value.clone());
                    }
                }
                Value::Array(items) => {
                    // An element set aside goes back at its index, if the
                    // list still reaches it.
                    if let Ok(i) = last.parse::<usize>() {
                        if e.filled.is_none() && i <= items.len() {
                            items.insert(i, e.value.clone());
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// Deserialize `value`, setting aside each value that fails to read and
/// trying again without it, so one value from a newer app costs its field
/// (which takes its default), not the config. Each set-aside value is
/// pushed to `unread` with the place it came from. An error that removing
/// a value cannot fix (a missing required field, a wrong shape at the
/// top) is returned as it was.
pub fn deserialize_lenient<T: DeserializeOwned>(mut value: Value, unread: &mut Vec<Entry>) -> Result<T, serde_json::Error> {
    // Each pass removes one value, so this bounds the passes; a config
    // with more unreadable values than this is not a newer config.
    for _ in 0..256 {
        match serde_path_to_error::deserialize::<_, T>(&value) {
            Ok(t) => return Ok(t),
            Err(e) => {
                let path = e.path().clone();
                let error = e.into_inner();
                let Some(pointer) = pointer_of(&path) else { return Err(error) };
                if error.to_string().starts_with("missing field") {
                    // A list item left without a required field (one set
                    // aside, or one a newer app renamed): the item goes
                    // aside whole, with what was already taken from it,
                    // and goes back at its index.
                    let in_list = pointer.rsplit_once('/').is_some_and(|(parent, last)| {
                        last.parse::<usize>().is_ok() && value.pointer(parent).is_some_and(Value::is_array)
                    });
                    if !in_list {
                        return Err(error);
                    }
                    let Some(mut item) = remove_at(&mut value, &pointer) else { return Err(error) };
                    let prefix = format!("{pointer}/");
                    let inner: Vec<Entry> = unread.iter().filter(|x| x.pointer.starts_with(&prefix)).cloned().collect();
                    unread.retain(|x| !x.pointer.starts_with(&prefix));
                    // What was taken from the item goes back into it first.
                    let taken = inner.into_iter().map(|x| Entry { pointer: x.pointer[pointer.len()..].to_string(), filled: None, ..x }).collect();
                    Unread { entries: taken, ..Default::default() }.restore(&mut item);
                    log::warn!("set aside a list item this version cannot read, at {pointer}: {error}");
                    unread.push(Entry { pointer, value: item, filled: None });
                    continue;
                }
                if pointer.is_empty() {
                    return Err(error);
                }
                let Some(removed) = remove_at(&mut value, &pointer) else { return Err(error) };
                log::warn!("set aside a config value this version cannot read, at {pointer}: {error}");
                unread.push(Entry { pointer, value: removed, filled: None });
            }
        }
    }
    serde_json::from_value(value)
}

/// After loading: what the loaded config writes at each set-aside place,
/// so a save can tell the default it took from the user's edit.
pub fn note_filled(entries: &mut [Entry], written: &Value) {
    for e in entries {
        // A list's element has no default to compare: the elements after
        // it moved up into its place. It goes back by index.
        let parent = e.pointer.rsplit_once('/').map_or("", |(p, _)| p);
        if e.filled.is_none() && written.pointer(parent).is_some_and(Value::is_object) {
            e.filled = written.pointer(&e.pointer).cloned();
        }
    }
}

/// A serde path as a JSON pointer. An externally tagged enum's variant is
/// a key in JSON; a segment serde could not name gives up.
fn pointer_of(path: &serde_path_to_error::Path) -> Option<String> {
    use serde_path_to_error::Segment;
    let mut out = String::new();
    for seg in path {
        out.push('/');
        match seg {
            Segment::Seq { index } => out.push_str(&index.to_string()),
            Segment::Map { key } => out.push_str(&escape(key)),
            Segment::Enum { variant } => out.push_str(&escape(variant)),
            Segment::Unknown => return None,
        }
    }
    Some(out)
}

fn escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

fn split(pointer: &str) -> Vec<String> {
    pointer.split('/').skip(1).map(|s| s.replace("~1", "/").replace("~0", "~")).collect()
}

/// Remove the value at `pointer`: a key from its object, an element from
/// its array. `None` if there is nothing there.
fn remove_at(root: &mut Value, pointer: &str) -> Option<Value> {
    let segments = split(pointer);
    let (last, parents) = segments.split_last()?;
    let mut node = root;
    for s in parents {
        node = match node {
            Value::Object(m) => m.get_mut(s.as_str())?,
            Value::Array(a) => a.get_mut(s.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    match node {
        Value::Object(m) => m.remove(last.as_str()),
        Value::Array(a) => {
            let i = last.parse::<usize>().ok()?;
            (i < a.len()).then(|| a.remove(i))
        }
        _ => None,
    }
}

/// The node at `segments`, creating missing objects on the way (a section
/// the writer left out as default still takes back what it held). Gives
/// up at a missing array element or a non-container.
fn walk_creating<'a>(root: &'a mut Value, segments: &[String]) -> Option<&'a mut Value> {
    let mut node = root;
    for s in segments {
        node = match node {
            Value::Object(m) => m.entry(s.clone()).or_insert_with(|| Value::Object(Default::default())),
            Value::Array(a) => a.get_mut(s.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(node)
}

/// The keys of `input` that `output` lacks, recursively through objects
/// and through arrays of the same length (a list's items, compared in
/// place).
fn diff(input: &Value, output: &Value, pointer: &mut String, out: &mut Vec<Entry>) {
    match (input, output) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, v) in a {
                let len = pointer.len();
                pointer.push('/');
                pointer.push_str(&escape(k));
                match b.get(k) {
                    None => out.push(Entry { pointer: pointer.clone(), value: v.clone(), filled: None }),
                    Some(w) => diff(v, w, pointer, out),
                }
                pointer.truncate(len);
            }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (i, (v, w)) in a.iter().zip(b).enumerate() {
                let len = pointer.len();
                pointer.push('/');
                pointer.push_str(&i.to_string());
                diff(v, w, pointer, out);
                pointer.truncate(len);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;

    #[derive(Debug, Deserialize, serde::Serialize, PartialEq, Default)]
    enum Mode {
        #[default]
        A,
        B,
    }

    #[derive(Debug, Deserialize, serde::Serialize, PartialEq)]
    struct Inner {
        #[serde(default)]
        mode: Mode,
        #[serde(default)]
        n: u32,
    }

    #[derive(Debug, Deserialize, serde::Serialize, PartialEq)]
    struct Outer {
        name: String,
        #[serde(default)]
        mode: Mode,
        inner: Inner,
        #[serde(default)]
        list: Vec<Mode>,
    }

    #[test]
    fn unreadable_values_are_set_aside_and_the_rest_loads() {
        let v = json!({"name": "x", "mode": "Hologram", "inner": {"mode": "B", "n": "seven"}, "list": ["A", "Z", "B"]});
        let mut entries = Vec::new();
        let o: Outer = deserialize_lenient(v, &mut entries).unwrap();
        assert_eq!(o, Outer { name: "x".into(), mode: Mode::A, inner: Inner { mode: Mode::B, n: 0 }, list: vec![Mode::A, Mode::B] });
        let places: Vec<&str> = entries.iter().map(|e| e.pointer.as_str()).collect();
        // In the order serde meets them: a JSON object's keys are sorted.
        assert_eq!(places, ["/inner/n", "/list/1", "/mode"]);
    }

    #[test]
    fn what_cannot_be_fixed_by_removal_still_fails() {
        let mut entries = Vec::new();
        assert!(deserialize_lenient::<Outer>(json!({"mode": "A", "inner": {}}), &mut entries).is_err(), "a missing required field");
        assert!(deserialize_lenient::<Outer>(json!([1, 2]), &mut entries).is_err(), "a wrong shape");
    }

    #[test]
    fn set_aside_values_go_back_unless_the_user_changed_the_field() {
        let input = json!({"name": "x", "mode": "Hologram", "inner": {"mode": "B", "n": 3}, "list": ["A", "Z"]});
        let mut unread = Unread::default();
        let mut o: Outer = deserialize_lenient(input, &mut unread.entries).unwrap();
        note_filled(&mut unread.entries, &serde_json::to_value(&o).unwrap());
        // Untouched: everything back as read.
        let mut out = serde_json::to_value(&o).unwrap();
        unread.restore(&mut out);
        assert_eq!(out, json!({"name": "x", "mode": "Hologram", "inner": {"mode": "B", "n": 3}, "list": ["A", "Z"]}));
        // The user picked a mode: theirs is kept.
        o.mode = Mode::B;
        let mut out = serde_json::to_value(&o).unwrap();
        unread.restore(&mut out);
        assert_eq!(out["mode"], json!("B"));
    }

    #[test]
    fn unknown_fields_are_found_and_written_back() {
        let input = json!({"name": "x", "inner": {"n": 1, "glow": 0.5}, "sparkle": {"on": true}, "list": ["A"]});
        let o: Outer = serde_json::from_value(input.clone()).unwrap();
        let mut unread = Unread::default();
        unread.capture_unknown_fields(&input, &serde_json::to_value(&o).unwrap());
        let places: Vec<&str> = unread.entries.iter().map(|e| e.pointer.as_str()).collect();
        assert_eq!(places, ["/inner/glow", "/sparkle"]);
        let mut out = serde_json::to_value(&o).unwrap();
        unread.restore(&mut out);
        assert_eq!(out["inner"]["glow"], json!(0.5));
        assert_eq!(out["sparkle"], json!({"on": true}));
        // A section the writer left out still takes back what it held.
        let mut out = json!({"name": "x"});
        unread.restore(&mut out);
        assert_eq!(out["inner"]["glow"], json!(0.5));
    }
}
