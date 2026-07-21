// ── region.rs ────────────────────────────────────────────────────────────────
// Arena-based memory management module.
// No unsafe or explicit lifetimes — "pointers" are integer indices.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RegionId {
    Global,
    Scoped,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObjectRef {
    pub region: RegionId,
    pub index: usize,
}

use crate::ast::{BlockStatement, Parameter};
use std::rc::Rc;

#[derive(Debug, Clone)]
pub enum OwnedValue {
    Integer(i64),
    Decimal(f64),
    Dec(rust_decimal::Decimal),
    Boolean(bool),
    Str(String),
    Array {
        element_type: Option<String>,
        elements: Vec<OwnedValue>,
    },
    Dict {
        key_type: String,
        value_type: String,
        entries: Vec<(OwnedValue, OwnedValue)>,
    },
    Function {
        return_type: Option<String>,
        parameters: Rc<Vec<Parameter>>,
        body: Rc<BlockStatement>, // Rc: cloning a function is O(1), not O(body_size)
        captured: Rc<Vec<(String, ObjectRef)>>,
        is_generator: bool,
        // Some(clase) si es una referencia a método ligada (`obj.metodo` sin paréntesis):
        // al invocarla hay que restaurar ese contexto de clase, o el cuerpo perdería el
        // acceso a los miembros privados de su propia clase.
        bound_class: Option<String>,
    },
    Instance {
        class_name: String,
        fields: Vec<(String, OwnedValue)>,
    },
    EnumVariant {
        enum_name: String,
        variant: String,
    },
    Set {
        elements: Vec<OwnedValue>,
    },
    Tensor {
        shape: Vec<usize>,
        data: Vec<f64>,
        tid: u64,        // stable identity — assigned at creation, survives extract/plant
    },
    DateTime { epoch_ms: i64, utc: bool },
    DateField { epoch_ms: i64, utc: bool, field: u8, value: i64 },
    Ptr(String), // pointer to a named variable
    Null,
}

impl OwnedValue {
    pub fn display_str(&self) -> String {
        match self {
            OwnedValue::Integer(i) => format!("{}", i),
            OwnedValue::Decimal(d) => {
                if d.fract() == 0.0 { format!("{:.1}", d) }
                else {
                    let s = format!("{:.10}", d);
                    s.trim_end_matches('0').trim_end_matches('.').to_string()
                }
            }
            // Exact: rust_decimal's Display preserves the scale (12.50, not 12.5).
            OwnedValue::Dec(d) => d.to_string(),
            OwnedValue::Boolean(b) => format!("{}", b),
            OwnedValue::Str(s) => s.clone(),
            OwnedValue::Array { elements, .. } => {
                let inner: Vec<String> = elements.iter().map(|v| v.display_str()).collect();
                format!("[{}]", inner.join(", "))
            }
            OwnedValue::Dict { entries, .. } => {
                let pairs: Vec<String> = entries.iter()
                    .map(|(k, v)| format!("{}: {}", k.display_str(), v.display_str()))
                    .collect();
                format!("{{{}}}", pairs.join(", "))
            }
            OwnedValue::Function { .. } => "Function".to_string(),
            OwnedValue::Instance { class_name, fields } => {
                let pairs: Vec<String> = fields.iter()
                    .map(|(n, v)| format!("{}: {}", n, v.display_str()))
                    .collect();
                format!("{}{{ {} }}", class_name, pairs.join(", "))
            }
            OwnedValue::EnumVariant { enum_name, variant } => format!("{}.{}", enum_name, variant),
            OwnedValue::Set { elements } => {
                let inner: Vec<String> = elements.iter().map(|v| v.display_str()).collect();
                format!("Set[{}]", inner.join(", "))
            }
            OwnedValue::Tensor { shape, data, .. } => format_tensor(shape, data),
            OwnedValue::DateTime { epoch_ms, utc } => format_datetime(*epoch_ms, *utc),
            OwnedValue::DateField { value, .. } => format!("{}", value),
            OwnedValue::Ptr(name) => format!("&{}", name),
            OwnedValue::Null => "null".to_string(),
        }
    }
}

/// Canonical key string used for ALL dict lookups (read, index-assign). Keeping
/// it in one place guarantees the hash index and the linear scans agree — e.g.
/// `d[5]` finds a `"5"` key, exactly like the historical inline conversions.
pub fn dict_key_str(k: &OwnedValue) -> String {
    match k {
        OwnedValue::Str(s) => s.clone(),
        OwnedValue::Integer(i) => i.to_string(),
        OwnedValue::Decimal(d) => d.to_string(),
        OwnedValue::Boolean(b) => b.to_string(),
        _ => format!("{:?}", k),
    }
}

/// Lazy hash index over a dict's entries: canonical key string → entry position.
/// It is a pure cache — `lookup` validates every hit against the real entries, so
/// code that mutates `entries` directly (get_mut paths) can never make it return
/// a wrong position; at worst it triggers a rebuild. Cloning a dict resets the
/// index (rebuilt on demand), keeping value-semantics copies cheap.
#[derive(Debug, Default)]
pub struct DictIndex {
    // (key → first position, entries.len() at build time, canonical key of the
    // last entry at build time). len+last-key stamp detects every mutation shape
    // the evaluator can produce (append, clear, and any future remove/re-push);
    // in-place VALUE updates keep keys at their positions and stay valid.
    #[allow(clippy::type_complexity)]
    cache: std::cell::RefCell<(std::collections::HashMap<String, usize>, usize, String)>,
}

impl Clone for DictIndex {
    fn clone(&self) -> Self {
        DictIndex::default()
    }
}

/// Below this size a linear scan beats building/consulting the hash map.
const DICT_INDEX_MIN: usize = 16;

impl DictIndex {
    /// Position of `key` in `entries`, or None. O(1) amortized on large dicts,
    /// plain linear scan on small ones (same cost as before the index existed).
    /// The cache can never produce a wrong answer: hits are validated against the
    /// real entry and false positions fall back to the linear scan; a key present
    /// in `entries` is always present in a freshly built cache, so a miss after
    /// a stamp-checked build is a true miss.
    pub fn lookup(&self, entries: &[(OwnedValue, OwnedValue)], key: &str) -> Option<usize> {
        if entries.len() < DICT_INDEX_MIN {
            return entries.iter().position(|(k, _)| dict_key_str(k) == key);
        }
        let mut guard = self.cache.borrow_mut();
        let (cache, built_len, built_last) = &mut *guard;
        let last_key = dict_key_str(&entries[entries.len() - 1].0);
        if *built_len != entries.len() || *built_last != last_key {
            cache.clear();
            for (i, (k, _)) in entries.iter().enumerate() {
                // First occurrence wins, matching linear find-first semantics.
                cache.entry(dict_key_str(k)).or_insert(i);
            }
            *built_len = entries.len();
            *built_last = last_key;
        }
        match cache.get(key) {
            Some(&i) if dict_key_str(&entries[i].0) == key => Some(i),
            // Validated-hit failure: positions shifted without touching len or
            // the last key. The key set is still covered by the cache, so the
            // linear scan below stays correct — just slower until a rebuild.
            Some(_) => entries.iter().position(|(k, _)| dict_key_str(k) == key),
            None => None,
        }
    }

    /// Keeps the cache warm across an append (`entries.push` right after a missed
    /// `lookup`). Without this, building a large dict key-by-key would rebuild the
    /// whole cache once per insert — O(N²) again. Only applies when the cache was
    /// valid for the pre-push length; otherwise the next lookup rebuilds anyway.
    pub fn record_append(&self, key: &str, pos: usize) {
        let mut guard = self.cache.borrow_mut();
        let (cache, built_len, built_last) = &mut *guard;
        if *built_len == pos && !cache.is_empty() {
            cache.entry(key.to_string()).or_insert(pos);
            *built_len = pos + 1;
            *built_last = key.to_string();
        }
    }
}

/// Canonical, TYPE-TAGGED fingerprint of a Set element, or None when the value
/// has no cheap identity. Mirrors `obj_data_eq` EXACTLY — which, unlike dict
/// keys, is strict about types (`5` and `"5"` are DIFFERENT set elements, and
/// compound values are never equal to anything). Edge cases handled: `-0.0`
/// folds into `0.0` (f64 `==` says they're equal), NaN gets no fingerprint
/// (NaN != NaN), and `dec` normalizes scale (1.50 == 1.5).
pub fn set_key_str(v: &OwnedValue) -> Option<String> {
    match v {
        OwnedValue::Str(s) => Some(format!("s:{}", s)),
        OwnedValue::Integer(i) => Some(format!("i:{}", i)),
        OwnedValue::Boolean(b) => Some(format!("b:{}", b)),
        OwnedValue::Dec(d) => Some(format!("d:{}", d.normalize())),
        OwnedValue::Decimal(f) if !f.is_nan() => {
            let f = if *f == 0.0 { 0.0 } else { *f };
            Some(format!("f:{}", f.to_bits()))
        }
        OwnedValue::Null => Some("n".to_string()),
        _ => None, // compound values: obj_data_eq never matches them — no identity
    }
}

/// Threshold shared with dicts: below this size a linear scan wins.
pub const SET_INDEX_MIN: usize = DICT_INDEX_MIN;

/// Lazy hash index over a Set's elements: fingerprint → position. Same design
/// and guarantees as `DictIndex` (pure cache, validated hits, len+last stamp,
/// Clone resets). Elements without a fingerprint are skipped when building —
/// they can never equal a fingerprintable probe under `obj_data_eq`.
#[derive(Debug, Default)]
pub struct SetIndex {
    #[allow(clippy::type_complexity)]
    cache: std::cell::RefCell<(std::collections::HashMap<String, usize>, usize, String)>,
}

impl Clone for SetIndex {
    fn clone(&self) -> Self {
        SetIndex::default()
    }
}

impl SetIndex {
    /// Position of the element whose fingerprint is `key`, or None. Callers use
    /// this only at/above SET_INDEX_MIN; small sets keep the linear scan.
    pub fn lookup(&self, elements: &[OwnedValue], key: &str) -> Option<usize> {
        let mut guard = self.cache.borrow_mut();
        let (cache, built_len, built_last) = &mut *guard;
        let last_key = elements.last().and_then(set_key_str).unwrap_or_default();
        if *built_len != elements.len() || *built_last != last_key {
            cache.clear();
            for (i, e) in elements.iter().enumerate() {
                if let Some(k) = set_key_str(e) {
                    cache.entry(k).or_insert(i);
                }
            }
            *built_len = elements.len();
            *built_last = last_key;
        }
        match cache.get(key) {
            Some(&i) if set_key_str(&elements[i]).as_deref() == Some(key) => Some(i),
            Some(_) => elements
                .iter()
                .position(|e| set_key_str(e).as_deref() == Some(key)),
            None => None,
        }
    }

    /// Keeps the cache warm across an append (`elements.push` right after a
    /// missed `lookup`), so building a big set via `add` in a loop stays O(1)
    /// per insert instead of rebuilding the cache once per insert.
    pub fn record_append(&self, key: &str, pos: usize) {
        let mut guard = self.cache.borrow_mut();
        let (cache, built_len, built_last) = &mut *guard;
        if *built_len == pos && !cache.is_empty() {
            cache.entry(key.to_string()).or_insert(pos);
            *built_len = pos + 1;
            *built_last = key.to_string();
        }
    }
}

#[derive(Debug, Clone)]
pub enum ObjectData {
    Integer(i64),
    Decimal(f64),
    Dec(rust_decimal::Decimal),
    Boolean(bool),
    Str(String),
    Array {
        element_type: Option<String>,
        elements: Vec<OwnedValue>,
    },
    Dict {
        key_type: String,
        value_type: String,
        entries: Vec<(OwnedValue, OwnedValue)>,
        index: DictIndex,
    },
    Function {
        return_type: Option<String>,
        parameters: Rc<Vec<Parameter>>,
        body: Rc<BlockStatement>, // Rc: cloning a function is O(1), not O(body_size)
        captured: Rc<Vec<(String, ObjectRef)>>,
        is_generator: bool,
        // Ver OwnedValue::Function.bound_class.
        bound_class: Option<String>,
    },
    // Fields stored as OwnedValues (embedded, arena-independent) to avoid cross-scope refs.
    Instance {
        class_name: String,
        fields: Vec<(String, OwnedValue)>,
    },
    EnumVariant {
        enum_name: String,
        variant: String,
    },
    Set {
        elements: Vec<OwnedValue>,
        index: SetIndex,
    },
    Tensor {
        shape: Vec<usize>,
        data: Vec<f64>,
        tid: u64,        // stable identity — assigned at creation, survives extract/plant
    },
    /// Internal date/time value. `epoch_ms` is the wall-clock instant frozen as
    /// milliseconds-since-epoch on the UTC timeline (deterministic, DST-free);
    /// `utc` records whether it originated as UTC (utcNow) or local (now/from) —
    /// used only for display labeling.
    DateTime { epoch_ms: i64, utc: bool },
    /// A single field of a DateTime (year/month/day/hour/minute/second/ms). Acts
    /// as an int under operators (coerced in eval_infix) but carries
    /// `.add/.reduce/.remove` methods that return a new DateTime. `field` is the
    /// field code: 0=year 1=month 2=day 3=hour 4=minute 5=second 6=ms.
    DateField { epoch_ms: i64, utc: bool, field: u8, value: i64 },
    Ptr(String), // pointer to a named variable
    Null,
}

impl std::fmt::Display for ObjectData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObjectData::Integer(i) => write!(f, "Integer({})", i),
            ObjectData::Decimal(d) => write!(f, "Decimal({})", d),
            ObjectData::Dec(d) => write!(f, "Dec({})", d),
            ObjectData::Boolean(b) => write!(f, "Boolean({})", b),
            ObjectData::Str(s) => write!(f, "String(\"{}\")", s),
            ObjectData::Array { element_type: Some(t), .. } => write!(f, "[{}]([...])", t),
            ObjectData::Array { .. } => write!(f, "Array([...])"),
            ObjectData::Dict { key_type, value_type, .. } => {
                write!(f, "Dict<{},{}>{{...}}", key_type, value_type)
            }
            ObjectData::Function { .. } => write!(f, "Function"),
            ObjectData::Instance { class_name, .. } => write!(f, "{}{{...}}", class_name),
            ObjectData::EnumVariant { enum_name, variant } => write!(f, "{}.{}", enum_name, variant),
            ObjectData::Set { .. } => write!(f, "Set{{...}}"),
            ObjectData::Tensor { shape, data, .. } => write!(f, "{}", format_tensor(shape, data)),
            ObjectData::DateTime { epoch_ms, utc } => write!(f, "DateTime({})", format_datetime(*epoch_ms, *utc)),
            ObjectData::DateField { value, .. } => write!(f, "DateField({})", value),
            ObjectData::Ptr(name) => write!(f, "Ptr(&{})", name),
            ObjectData::Null => write!(f, "Null"),
        }
    }
}

pub struct Arena {
    storage: Vec<ObjectData>,
}

impl Arena {
    pub fn new() -> Self {
        Arena { storage: Vec::with_capacity(64) }
    }

    pub fn alloc(&mut self, data: ObjectData) -> usize {
        let idx = self.storage.len();
        self.storage.push(data);
        idx
    }

    pub fn get(&self, index: usize) -> Option<&ObjectData> {
        self.storage.get(index)
    }

    /// Mutable access to a slot. Lets callers mutate a container in place
    /// (e.g. one array element) instead of cloning + `update`ing the whole value.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut ObjectData> {
        self.storage.get_mut(index)
    }

    pub fn update(&mut self, index: usize, data: ObjectData) {
        if let Some(slot) = self.storage.get_mut(index) {
            *slot = data;
        }
    }

    pub fn watermark(&self) -> usize {
        self.storage.len()
    }

    pub fn reset_to(&mut self, mark: usize) {
        self.storage.truncate(mark);
    }
}

/// Render a DateTime as ISO 8601: `YYYY-MM-DDTHH:MM:SS` (with `.mmm` when the
/// millisecond component is non-zero), suffixed with `Z` when it is a UTC value.
/// `epoch_ms` is interpreted on the UTC timeline (the value was frozen that way
/// at construction), so this is deterministic regardless of the host timezone.
pub fn format_datetime(epoch_ms: i64, utc: bool) -> String {
    use chrono::{DateTime, Utc, Datelike, Timelike};
    let dt: DateTime<Utc> = DateTime::<Utc>::from_timestamp_millis(epoch_ms)
        .unwrap_or_else(|| DateTime::<Utc>::from_timestamp_millis(0).unwrap());
    let ms = dt.timestamp_subsec_millis();
    let base = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        dt.year(), dt.month(), dt.day(), dt.hour(), dt.minute(), dt.second()
    );
    let base = if ms != 0 { format!("{}.{:03}", base, ms) } else { base };
    if utc { format!("{}Z", base) } else { base }
}

pub fn format_tensor(shape: &[usize], data: &[f64]) -> String {
    fn fmt_val(v: f64) -> String {
        if v.fract() == 0.0 {
            format!("{:.1}", v)
        } else {
            let s = format!("{:.6}", v);
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        }
    }
    fn nest(shape: &[usize], data: &[f64], off: usize) -> String {
        if shape.len() == 1 {
            let vs: Vec<String> = (0..shape[0]).map(|i| fmt_val(data[off + i])).collect();
            format!("[{}]", vs.join(", "))
        } else {
            let stride: usize = shape[1..].iter().product();
            let rows: Vec<String> = (0..shape[0])
                .map(|i| nest(&shape[1..], data, off + i * stride))
                .collect();
            format!("[{}]", rows.join(", "))
        }
    }
    if shape.is_empty() {
        return "Tensor([])".to_string();
    }
    format!("Tensor({})", nest(shape, data, 0))
}

impl ObjectData {
    pub fn type_name(&self) -> &str {
        match self {
            ObjectData::Integer(_) => "int",
            ObjectData::Decimal(_) => "decimal",
            ObjectData::Dec(_) => "dec",
            ObjectData::Boolean(_) => "bool",
            ObjectData::Str(_) => "string",
            ObjectData::Array { .. } => "array",
            ObjectData::Dict { .. } => "dict",
            ObjectData::Function { .. } => "function",
            ObjectData::Instance { class_name, .. } => class_name.as_str(),
            ObjectData::EnumVariant { enum_name, .. } => enum_name.as_str(),
            ObjectData::Set { .. } => "Set",
            ObjectData::Tensor { .. } => "Tensor",
            ObjectData::DateTime { .. } => "DateTime",
            // DateField acts as an int under operators, so report it as such.
            ObjectData::DateField { .. } => "int",
            ObjectData::Ptr(_) => "ptr",
            ObjectData::Null => "null",
        }
    }
}
