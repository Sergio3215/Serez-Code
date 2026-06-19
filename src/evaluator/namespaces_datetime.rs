// DateTime namespace — immutable calendar date/time built on `chrono`.
//
// Design (see project-datetime-plan):
//   • DateTime is an internal value `{ epoch_ms, utc }`. `epoch_ms` is the
//     wall-clock instant frozen as milliseconds on the UTC timeline, which makes
//     every operation deterministic regardless of the host timezone / DST. The
//     `utc` flag only records the origin (utcNow vs now/from) for display.
//   • `date.<field>` returns a DateField, which acts as an int under operators
//     (coerced in eval_infix) yet carries `.add(n)/.reduce(n)/.remove(n)` that
//     return a *new* DateTime — the ergonomic API `date.month.reduce(1)`.
//   • Arithmetic: day/hour/minute/second/ms operate on the instant (exact carry);
//     month/year operate field-wise with end-of-month day clamping.
//
// Permission: only the clock-reading entry points (`now`, `utcNow`) require the
// `Time` permission. Pure construction (`from`, `fromEpoch`) and any operation on
// an existing DateTime (fields, arithmetic, formatting) need no permission.

use crate::ast;
use crate::region::{ObjectData, OwnedValue};
use super::EvalResult;
use chrono::{DateTime, Utc, Local, NaiveDate, NaiveDateTime, Datelike, Timelike};

// Field codes carried by ObjectData::DateField.
const F_YEAR: u8 = 0;
const F_MONTH: u8 = 1;
const F_DAY: u8 = 2;
const F_HOUR: u8 = 3;
const F_MINUTE: u8 = 4;
const F_SECOND: u8 = 5;
const F_MS: u8 = 6;

// ── Pure date helpers (no Evaluator state) ───────────────────────────────────

/// Decode an epoch-ms value into its naive calendar parts on the UTC timeline.
fn parts(epoch_ms: i64) -> NaiveDateTime {
    DateTime::<Utc>::from_timestamp_millis(epoch_ms)
        .unwrap_or_else(|| DateTime::<Utc>::from_timestamp_millis(0).unwrap())
        .naive_utc()
}

/// Encode naive calendar parts (treated as UTC) back into epoch-ms.
fn to_epoch(ndt: NaiveDateTime) -> i64 {
    ndt.and_utc().timestamp_millis()
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if is_leap(y) { 29 } else { 28 },
        _ => 30,
    }
}

fn millis_of(ndt: &NaiveDateTime) -> u32 {
    (ndt.nanosecond() / 1_000_000).min(999)
}

/// Current integer value of `field` for the datetime at `epoch_ms`.
fn field_value(epoch_ms: i64, field: u8) -> i64 {
    let ndt = parts(epoch_ms);
    match field {
        F_YEAR => ndt.year() as i64,
        F_MONTH => ndt.month() as i64,
        F_DAY => ndt.day() as i64,
        F_HOUR => ndt.hour() as i64,
        F_MINUTE => ndt.minute() as i64,
        F_SECOND => ndt.second() as i64,
        F_MS => millis_of(&ndt) as i64,
        _ => 0,
    }
}

/// Add `delta` to one field of the datetime at `epoch_ms`, returning a new
/// epoch-ms — or `None` if the result overflows the representable date range
/// (rather than silently saturating to 1970). day/hour/minute/second/ms shift
/// the instant directly; month/year are computed field-wise and clamp the day
/// to the last valid day of the resulting month.
fn add_to_field(epoch_ms: i64, field: u8, delta: i64) -> Option<i64> {
    let new_epoch = match field {
        F_MS => epoch_ms.checked_add(delta)?,
        F_SECOND => epoch_ms.checked_add(delta.checked_mul(1_000)?)?,
        F_MINUTE => epoch_ms.checked_add(delta.checked_mul(60_000)?)?,
        F_HOUR => epoch_ms.checked_add(delta.checked_mul(3_600_000)?)?,
        F_DAY => epoch_ms.checked_add(delta.checked_mul(86_400_000)?)?,
        F_MONTH | F_YEAR => {
            let ndt = parts(epoch_ms);
            let delta_months = if field == F_YEAR { delta.checked_mul(12)? } else { delta };
            let total = (ndt.year() as i64)
                .checked_mul(12)?
                .checked_add(ndt.month() as i64 - 1)?
                .checked_add(delta_months)?;
            let new_y_i64 = total.div_euclid(12);
            if new_y_i64 < i32::MIN as i64 || new_y_i64 > i32::MAX as i64 { return None; }
            let new_y = new_y_i64 as i32;
            let new_m = (total.rem_euclid(12) + 1) as u32;
            let new_d = (ndt.day()).min(days_in_month(new_y, new_m));
            let out = NaiveDate::from_ymd_opt(new_y, new_m, new_d)?
                .and_hms_milli_opt(ndt.hour(), ndt.minute(), ndt.second(), millis_of(&ndt))?;
            return Some(to_epoch(out));
        }
        _ => return Some(epoch_ms),
    };
    // Reject instants beyond chrono's representable calendar range.
    if DateTime::<Utc>::from_timestamp_millis(new_epoch).is_none() { return None; }
    Some(new_epoch)
}

/// Calendar fields exposed by object-destructuring of a DateTime.
pub(crate) fn datetime_field_entries(epoch_ms: i64) -> Vec<(OwnedValue, OwnedValue)> {
    let ndt = parts(epoch_ms);
    vec![
        (OwnedValue::Str("year".into()),      OwnedValue::Integer(ndt.year() as i64)),
        (OwnedValue::Str("month".into()),     OwnedValue::Integer(ndt.month() as i64)),
        (OwnedValue::Str("day".into()),       OwnedValue::Integer(ndt.day() as i64)),
        (OwnedValue::Str("hour".into()),      OwnedValue::Integer(ndt.hour() as i64)),
        (OwnedValue::Str("minute".into()),    OwnedValue::Integer(ndt.minute() as i64)),
        (OwnedValue::Str("second".into()),    OwnedValue::Integer(ndt.second() as i64)),
        (OwnedValue::Str("ms".into()),        OwnedValue::Integer(millis_of(&ndt) as i64)),
        (OwnedValue::Str("weekday".into()),     OwnedValue::Integer(ndt.weekday().number_from_monday() as i64)),
        (OwnedValue::Str("dayOfYear".into()),   OwnedValue::Integer(ndt.ordinal() as i64)),
        (OwnedValue::Str("daysInMonth".into()), OwnedValue::Integer(days_in_month(ndt.year(), ndt.month()) as i64)),
    ]
}

/// Render a DateTime with a moment.js-style pattern. Tokens: YYYY/YY, MM/M,
/// DD/D, HH/H (24h), hh/h (12h), mm/m, ss/s, SSS (ms), A (AM/PM). Text wrapped
/// in `[...]` is emitted literally (brackets removed); any other non-token
/// character passes through as-is.
fn format_pattern(epoch_ms: i64, pat: &str) -> String {
    let ndt = parts(epoch_ms);
    let (y, mo, d) = (ndt.year(), ndt.month(), ndt.day());
    let (h, mi, s) = (ndt.hour(), ndt.minute(), ndt.second());
    let ms = millis_of(&ndt);
    let h12 = { let x = h % 12; if x == 0 { 12 } else { x } };
    let chars: Vec<char> = pat.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '[' {
            // Literal escape: copy verbatim until the matching ']' (brackets dropped).
            let mut j = i + 1;
            while j < chars.len() && chars[j] != ']' { out.push(chars[j]); j += 1; }
            i = if j < chars.len() { j + 1 } else { j };
            continue;
        }
        if "YMDHhmsSA".contains(c) {
            let mut n = 1;
            while i + n < chars.len() && chars[i + n] == c { n += 1; }
            match (c, n) {
                ('Y', 4) => out.push_str(&format!("{:04}", y)),
                ('Y', 2) => out.push_str(&format!("{:02}", (y % 100 + 100) % 100)),
                ('Y', _) => out.push_str(&format!("{}", y)),
                ('M', 2) => out.push_str(&format!("{:02}", mo)),
                ('M', _) => out.push_str(&format!("{}", mo)),
                ('D', 2) => out.push_str(&format!("{:02}", d)),
                ('D', _) => out.push_str(&format!("{}", d)),
                ('H', 2) => out.push_str(&format!("{:02}", h)),
                ('H', _) => out.push_str(&format!("{}", h)),
                ('h', 2) => out.push_str(&format!("{:02}", h12)),
                ('h', _) => out.push_str(&format!("{}", h12)),
                ('m', 2) => out.push_str(&format!("{:02}", mi)),
                ('m', _) => out.push_str(&format!("{}", mi)),
                ('s', 2) => out.push_str(&format!("{:02}", s)),
                ('s', _) => out.push_str(&format!("{}", s)),
                ('S', _) => out.push_str(&format!("{:03}", ms)),
                ('A', _) => out.push_str(if h < 12 { "AM" } else { "PM" }),
                _ => { for _ in 0..n { out.push(c); } }
            }
            i += n;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

impl super::Evaluator {
    // ── Static namespace: DateTime.now / utcNow / from / fromEpoch ────────────
    pub(super) fn eval_datetime_namespace(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        match dot_call.method.as_str() {
            "now" => {
                if !self.permissions.contains("Time") {
                    eprintln!("❌ ERROR: 'DateTime.now' requires permission 'Time' — declare it in serez.json (\"permissions\": [\"Time\", ...]) or with `use permissions {{ Time }}`");
                    return EvalResult::Error;
                }
                let epoch_ms = to_epoch(Local::now().naive_local());
                EvalResult::Value(self.alloc(ObjectData::DateTime { epoch_ms, utc: false }))
            }
            "utcNow" => {
                if !self.permissions.contains("Time") {
                    eprintln!("❌ ERROR: 'DateTime.utcNow' requires permission 'Time' — declare it in serez.json (\"permissions\": [\"Time\", ...]) or with `use permissions {{ Time }}`");
                    return EvalResult::Error;
                }
                let epoch_ms = Utc::now().timestamp_millis();
                EvalResult::Value(self.alloc(ObjectData::DateTime { epoch_ms, utc: true }))
            }
            "from" => {
                // DateTime.from(year, month, day, [hour, minute, second, ms])
                let nums = match self.collect_int_args(dot_call) {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                if nums.len() < 3 || nums.len() > 7 {
                    eprintln!("❌ ERROR: DateTime.from(year, month, day, [hour, minute, second, ms]) takes 3 to 7 integers");
                    return EvalResult::Error;
                }
                let get = |i: usize, d: i64| -> i64 { *nums.get(i).unwrap_or(&d) };
                let (y, mo, da) = (get(0, 1970), get(1, 1), get(2, 1));
                let (h, mi, s, ms) = (get(3, 0), get(4, 0), get(5, 0), get(6, 0));
                let valid = (1..=9999).contains(&y)
                    && (1..=12).contains(&mo)
                    && (1..=31).contains(&da)
                    && (0..=23).contains(&h)
                    && (0..=59).contains(&mi)
                    && (0..=59).contains(&s)
                    && (0..=999).contains(&ms);
                if !valid {
                    eprintln!("❌ ERROR: DateTime.from received an out-of-range field (year 1-9999, month 1-12, day 1-31, hour 0-23, min/sec 0-59, ms 0-999)");
                    return EvalResult::Error;
                }
                let built = NaiveDate::from_ymd_opt(y as i32, mo as u32, da as u32)
                    .and_then(|d| d.and_hms_milli_opt(h as u32, mi as u32, s as u32, ms as u32));
                match built {
                    Some(ndt) => {
                        let epoch_ms = to_epoch(ndt);
                        EvalResult::Value(self.alloc(ObjectData::DateTime { epoch_ms, utc: false }))
                    }
                    None => {
                        eprintln!("❌ ERROR: DateTime.from received an invalid calendar date ({:04}-{:02}-{:02})", y, mo, da);
                        EvalResult::Error
                    }
                }
            }
            "fromEpoch" | "fromTimestamp" => {
                let nums = match self.collect_int_args(dot_call) {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                if nums.len() != 1 {
                    eprintln!("❌ ERROR: DateTime.fromEpoch(milliseconds) requires 1 integer");
                    return EvalResult::Error;
                }
                if DateTime::<Utc>::from_timestamp_millis(nums[0]).is_none() {
                    eprintln!("❌ ERROR: DateTime.fromEpoch received an out-of-range timestamp (ms): {}", nums[0]);
                    return EvalResult::Error;
                }
                EvalResult::Value(self.alloc(ObjectData::DateTime { epoch_ms: nums[0], utc: true }))
            }
            other => {
                eprintln!("❌ ERROR: Unknown DateTime method '{}' (expected now/utcNow/from/fromEpoch)", other);
                EvalResult::Error
            }
        }
    }

    // Evaluate every argument of a dot-call as an integer.
    fn collect_int_args(&mut self, dot_call: &ast::DotCallExpression) -> Result<Vec<i64>, EvalResult> {
        let mut out = Vec::with_capacity(dot_call.arguments.len());
        for arg in &dot_call.arguments {
            let r = match self.eval_expression(arg) {
                EvalResult::Value(r) => r,
                EvalResult::Throw(v) => return Err(EvalResult::Throw(v)),
                _ => return Err(EvalResult::Error),
            };
            match self.resolve(r) {
                Some(ObjectData::Integer(n)) => out.push(*n),
                // A DateField passed as an argument acts as its int value.
                Some(ObjectData::DateField { value, .. }) => out.push(*value),
                _ => {
                    eprintln!("❌ ERROR: DateTime expects integer arguments");
                    return Err(EvalResult::Error);
                }
            }
        }
        Ok(out)
    }

    // ── Instance: field getters, formatting, conversions ─────────────────────
    pub(super) fn eval_datetime_method(
        &mut self,
        epoch_ms: i64,
        utc: bool,
        dot_call: &ast::DotCallExpression,
    ) -> EvalResult {
        let field = match dot_call.method.as_str() {
            "year" => Some(F_YEAR),
            "month" => Some(F_MONTH),
            "day" => Some(F_DAY),
            "hour" => Some(F_HOUR),
            "minute" => Some(F_MINUTE),
            "second" => Some(F_SECOND),
            "ms" | "millisecond" => Some(F_MS),
            _ => None,
        };
        if let Some(f) = field {
            let value = field_value(epoch_ms, f);
            return EvalResult::Value(self.alloc(ObjectData::DateField { epoch_ms, utc, field: f, value }));
        }

        let ndt = parts(epoch_ms);
        match dot_call.method.as_str() {
            // Read-only derived ints.
            "weekday" => EvalResult::Value(self.alloc(ObjectData::Integer(ndt.weekday().number_from_monday() as i64))),
            "dayOfYear" => EvalResult::Value(self.alloc(ObjectData::Integer(ndt.ordinal() as i64))),
            "daysInMonth" => EvalResult::Value(self.alloc(ObjectData::Integer(days_in_month(ndt.year(), ndt.month()) as i64))),
            "isLeapYear" => EvalResult::Value(self.alloc(ObjectData::Boolean(is_leap(ndt.year())))),
            "isUtc" => EvalResult::Value(self.alloc(ObjectData::Boolean(utc))),
            "timestamp" | "toEpoch" | "epochMillis" => EvalResult::Value(self.alloc(ObjectData::Integer(epoch_ms))),
            "toString" | "iso" => {
                let s = crate::region::format_datetime(epoch_ms, utc);
                EvalResult::Value(self.alloc(ObjectData::Str(s)))
            }
            "format" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: DateTime.format(pattern) requires 1 string argument");
                    return EvalResult::Error;
                }
                let r = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let pat = match self.resolve(r) {
                    Some(ObjectData::Str(s)) => s.clone(),
                    _ => { eprintln!("❌ ERROR: DateTime.format(pattern) requires a string pattern"); return EvalResult::Error; }
                };
                EvalResult::Value(self.alloc(ObjectData::Str(format_pattern(epoch_ms, &pat))))
            }
            other => {
                eprintln!("❌ ERROR: Unknown DateTime field/method '{}'", other);
                EvalResult::Error
            }
        }
    }

    // ── DateField: immutable arithmetic returning a new DateTime ──────────────
    pub(super) fn eval_datefield_method(
        &mut self,
        epoch_ms: i64,
        utc: bool,
        field: u8,
        value: i64,
        dot_call: &ast::DotCallExpression,
    ) -> EvalResult {
        match dot_call.method.as_str() {
            "add" | "reduce" | "remove" => {
                let nums = match self.collect_int_args(dot_call) {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                if nums.len() != 1 {
                    eprintln!("❌ ERROR: DateField.{}(n) requires 1 integer", dot_call.method);
                    return EvalResult::Error;
                }
                let signed = if dot_call.method == "add" { nums[0] } else { nums[0].checked_neg().unwrap_or(i64::MAX) };
                let new_epoch = match add_to_field(epoch_ms, field, signed) {
                    Some(e) => e,
                    None => {
                        eprintln!("❌ ERROR: DateField.{}({}) overflowed the representable date range", dot_call.method, nums[0]);
                        return EvalResult::Error;
                    }
                };
                EvalResult::Value(self.alloc(ObjectData::DateTime { epoch_ms: new_epoch, utc }))
            }
            "value" | "toInt" => EvalResult::Value(self.alloc(ObjectData::Integer(value))),
            "toString" => EvalResult::Value(self.alloc(ObjectData::Str(format!("{}", value)))),
            other => {
                eprintln!("❌ ERROR: Unknown DateField method '{}' (expected add/reduce/remove/value)", other);
                EvalResult::Error
            }
        }
    }
}
