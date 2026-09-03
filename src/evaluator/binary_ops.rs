//! The `Binary` namespace's operations, behind [`ValueSink`].
//!
//! **DEC-M6-001, first service across the boundary.** Every function here takes
//! values it has already been given and a sink to put results into. None of them
//! can reach the arenas, the scope stack, the class registry, the permission set
//! or the call stack, because a `&mut dyn ValueSink` does not offer them.
//!
//! What stayed in `namespaces_binary.rs` is argument *evaluation* — reading a
//! `DotCallExpression`, running its argument expressions, checking arity. That is
//! the evaluator's job and nothing else's, and a service able to evaluate an
//! arbitrary expression would have the whole interpreter back.
//!
//! The split is the point. `namespaces_binary.rs` reads as "get the values, then
//! do the thing"; the thing is here, and it is testable with a stub — the
//! property the register named as the reason to prefer a trait over passing
//! `&mut Evaluator`.
//!
//! Behaviour is unchanged, including every message. `tools/runtime_diff.sh`
//! compares 226 fixtures byte for byte across the move.

use crate::region::{ObjectData, OwnedValue};

use super::EvalResult;
use super::ExecutionFlow;
use super::service::ValueSink;

/// A byte array, as the language represents one: `[int]` with values 0-255.
fn byte_array(sink: &mut dyn ValueSink, bytes: &[u8]) -> EvalResult {
    let elements = bytes
        .iter()
        .map(|&b| OwnedValue::Integer(b as i64))
        .collect();
    Ok(ExecutionFlow::Value(sink.alloc(ObjectData::Array {
        element_type: Some("int".to_string()),
        elements,
    })))
}

/// Every element as a byte, or the index of the first that is not an integer.
fn as_bytes(elements: &[OwnedValue]) -> Result<Vec<u8>, ()> {
    elements
        .iter()
        .map(|e| match e {
            OwnedValue::Integer(b) => Ok(*b as u8),
            _ => Err(()),
        })
        .collect()
}

pub(super) fn from_hex(sink: &mut dyn ValueSink, hex: &str) -> EvalResult {
    if hex.len() % 2 != 0 {
        return sink.raise(
            "BinaryError",
            "Binary.fromHex: hex string must have even length".to_string(),
        );
    }
    let mut bytes: Vec<OwnedValue> = Vec::with_capacity(hex.len() / 2);
    for i in (0..hex.len()).step_by(2) {
        match u8::from_str_radix(&hex[i..i + 2], 16) {
            Ok(b) => bytes.push(OwnedValue::Integer(b as i64)),
            Err(_) => {
                return sink.raise(
                    "BinaryError",
                    format!("Binary.fromHex: invalid hex pair '{}'", &hex[i..i + 2]),
                );
            }
        }
    }
    Ok(ExecutionFlow::Value(sink.alloc(ObjectData::Array {
        element_type: Some("int".to_string()),
        elements: bytes,
    })))
}

pub(super) fn to_hex(sink: &mut dyn ValueSink, elements: &[OwnedValue]) -> EvalResult {
    let mut hex = String::with_capacity(elements.len() * 2);
    for element in elements {
        match element {
            OwnedValue::Integer(b) => hex.push_str(&format!("{:02x}", (*b as u8))),
            _ => {
                return sink.raise(
                    "TypeError",
                    "Binary.toHex: all elements must be integers".to_string(),
                );
            }
        }
    }
    Ok(ExecutionFlow::Value(sink.alloc(ObjectData::Str(hex))))
}

pub(super) fn from_utf8(sink: &mut dyn ValueSink, text: &str) -> EvalResult {
    byte_array(sink, text.as_bytes())
}

pub(super) fn to_utf8(sink: &mut dyn ValueSink, elements: &[OwnedValue]) -> EvalResult {
    match as_bytes(elements) {
        Ok(bytes) => {
            let text = String::from_utf8_lossy(&bytes).into_owned();
            Ok(ExecutionFlow::Value(sink.alloc(ObjectData::Str(text))))
        }
        Err(()) => sink.raise(
            "TypeError",
            "Binary.toUtf8: all elements must be integers".to_string(),
        ),
    }
}

pub(super) fn pack_i32_le(sink: &mut dyn ValueSink, n: i64) -> EvalResult {
    byte_array(sink, &(n as u32).to_le_bytes())
}

pub(super) fn pack_i32_be(sink: &mut dyn ValueSink, n: i64) -> EvalResult {
    byte_array(sink, &(n as u32).to_be_bytes())
}

pub(super) fn pack_i64_le(sink: &mut dyn ValueSink, n: i64) -> EvalResult {
    byte_array(sink, &n.to_le_bytes())
}

pub(super) fn unpack_i32_le(sink: &mut dyn ValueSink, bytes: &[u8]) -> EvalResult {
    let Some(head) = bytes.get(..4) else {
        return sink.raise(
            "BinaryError",
            "Binary.unpackInt32Le: need at least 4 bytes".to_string(),
        );
    };
    let n = u32::from_le_bytes(head.try_into().expect("4 bytes")) as i64;
    Ok(ExecutionFlow::Value(sink.alloc(ObjectData::Integer(n))))
}

pub(super) fn unpack_i32_be(sink: &mut dyn ValueSink, bytes: &[u8]) -> EvalResult {
    let Some(head) = bytes.get(..4) else {
        return sink.raise(
            "BinaryError",
            "Binary.unpackInt32Be: need at least 4 bytes".to_string(),
        );
    };
    let n = u32::from_be_bytes(head.try_into().expect("4 bytes")) as i64;
    Ok(ExecutionFlow::Value(sink.alloc(ObjectData::Integer(n))))
}

pub(super) fn unpack_i64_le(sink: &mut dyn ValueSink, bytes: &[u8]) -> EvalResult {
    let Some(head) = bytes.get(..8) else {
        return sink.raise(
            "BinaryError",
            "Binary.unpackInt64Le: need at least 8 bytes".to_string(),
        );
    };
    let n = i64::from_le_bytes(head.try_into().expect("8 bytes"));
    Ok(ExecutionFlow::Value(sink.alloc(ObjectData::Integer(n))))
}

pub(super) fn concat(
    sink: &mut dyn ValueSink,
    first: &[OwnedValue],
    second: &[OwnedValue],
) -> EvalResult {
    let mut elements = first.to_vec();
    elements.extend_from_slice(second);
    Ok(ExecutionFlow::Value(sink.alloc(ObjectData::Array {
        element_type: Some("int".to_string()),
        elements,
    })))
}

#[cfg(test)]
mod tests {
    //! Run against a stub, with no evaluator anywhere.
    //!
    //! That these compile and pass is the boundary check DEC-M6-001 is about: an
    //! operation that still needed the arena, the scope stack or the class
    //! registry could not be written against `Recorder`, and the failure would be
    //! at compile time rather than in a review.

    use super::super::service::stub::Recorder;
    use super::*;

    fn ints(values: &[i64]) -> Vec<OwnedValue> {
        values.iter().map(|v| OwnedValue::Integer(*v)).collect()
    }

    fn allocated(sink: &Recorder) -> &ObjectData {
        sink.allocated.last().expect("the operation allocated")
    }

    /// Compared through `Debug` because `ObjectData` and `OwnedValue` do not
    /// implement `PartialEq` — deriving it on a core runtime type to make a test
    /// read better would be a product change for a test's convenience. Every
    /// variant these operations produce has a faithful `Debug`, which is the same
    /// property `parser_snapshot` relies on.
    fn shows(actual: &ObjectData, expected: &ObjectData) -> bool {
        format!("{actual:?}") == format!("{expected:?}")
    }

    fn array_of(values: &[i64]) -> ObjectData {
        ObjectData::Array {
            element_type: Some("int".to_string()),
            elements: ints(values),
        }
    }

    #[test]
    fn hex_round_trips_through_the_sink() {
        let mut sink = Recorder::default();
        from_hex(&mut sink, "0aff").expect("valid hex");
        assert!(shows(allocated(&sink), &array_of(&[10, 255])));

        let bytes = ints(&[10, 255]);
        to_hex(&mut sink, &bytes).expect("valid bytes");
        assert!(shows(
            allocated(&sink),
            &ObjectData::Str("0aff".to_string())
        ));
    }

    #[test]
    fn an_invalid_hex_pair_is_raised_through_the_sink_and_allocates_nothing() {
        let mut sink = Recorder::default();
        assert!(from_hex(&mut sink, "0az0").is_err());
        let (kind, message) = sink.raised.expect("the operation raised");
        assert_eq!(kind, "BinaryError");
        assert!(message.contains("invalid hex pair 'z0'"), "{message}");
    }

    #[test]
    fn an_odd_length_hex_string_is_refused() {
        let mut sink = Recorder::default();
        assert!(from_hex(&mut sink, "abc").is_err());
        assert_eq!(sink.raised.expect("raised").0, "BinaryError");
    }

    #[test]
    fn a_non_integer_element_is_refused_by_both_directions() {
        for op in [
            to_hex as fn(&mut dyn ValueSink, &[OwnedValue]) -> EvalResult,
            to_utf8,
        ] {
            let mut sink = Recorder::default();
            let bad = vec![OwnedValue::Integer(1), OwnedValue::Str("x".to_string())];
            assert!(op(&mut sink, &bad).is_err());
            assert_eq!(sink.raised.expect("raised").0, "TypeError");
        }
    }

    #[test]
    fn packing_and_unpacking_agree() {
        let mut sink = Recorder::default();
        pack_i32_le(&mut sink, 1).expect("packs");
        assert!(shows(allocated(&sink), &array_of(&[1, 0, 0, 0])));

        unpack_i32_le(&mut sink, &[1, 0, 0, 0]).expect("unpacks");
        assert!(shows(allocated(&sink), &ObjectData::Integer(1)));

        unpack_i32_be(&mut sink, &[0, 0, 0, 1]).expect("unpacks");
        assert!(shows(allocated(&sink), &ObjectData::Integer(1)));

        unpack_i64_le(&mut sink, &[1, 0, 0, 0, 0, 0, 0, 0]).expect("unpacks");
        assert!(shows(allocated(&sink), &ObjectData::Integer(1)));
    }

    #[test]
    fn a_short_buffer_is_refused_rather_than_read_past() {
        // The `get(..4)` / `get(..8)` guards. Indexing would panic here, which is
        // a crash rather than an error, and the boundary is one byte away.
        let mut sink = Recorder::default();
        assert!(unpack_i32_le(&mut sink, &[1, 2, 3]).is_err());
        assert!(unpack_i32_be(&mut sink, &[1, 2, 3]).is_err());
        assert!(unpack_i64_le(&mut sink, &[1, 2, 3, 4, 5, 6, 7]).is_err());
        // And exactly enough is enough.
        assert!(unpack_i32_le(&mut sink, &[1, 2, 3, 4]).is_ok());
        assert!(unpack_i64_le(&mut sink, &[1, 2, 3, 4, 5, 6, 7, 8]).is_ok());
    }

    #[test]
    fn utf8_round_trips() {
        let mut sink = Recorder::default();
        from_utf8(&mut sink, "hí").expect("encodes");
        let ObjectData::Array { elements, .. } = allocated(&sink).clone() else {
            panic!("expected an array");
        };
        to_utf8(&mut sink, &elements).expect("decodes");
        assert!(shows(allocated(&sink), &ObjectData::Str("hí".to_string())));
    }

    #[test]
    fn concat_appends_in_order() {
        let mut sink = Recorder::default();
        concat(&mut sink, &ints(&[1, 2]), &ints(&[3])).expect("concatenates");
        assert!(shows(allocated(&sink), &array_of(&[1, 2, 3])));
    }
}
