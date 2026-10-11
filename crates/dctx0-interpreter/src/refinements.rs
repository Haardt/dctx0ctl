//! Executable refinements: reference UTF-16 lengths and reviewed text formats.
use crate::contracts::ValueContract as C;
use dctx0_core::{VerificationIssue, VerificationValue as V};
fn unknown(message: &str) -> C {
    C::Unknown {
        issue: VerificationIssue {
            code: "INVALID_REFINEMENT".into(),
            message: message.into(),
            span: None,
        },
    }
}
fn verdict(valid: bool, message: &str) -> C {
    if valid {
        C::Accepted
    } else {
        C::Rejected {
            reason: message.into(),
        }
    }
}
pub(crate) fn length(terms: &[&str], value: &V) -> C {
    let text = terms.join("");
    let Some((min, max)) = text.split_once("..") else {
        return unknown("Length needs minimum..maximum");
    };
    let (Ok(min), Ok(max)) = (min.parse::<usize>(), max.parse::<usize>()) else {
        return unknown("Length bounds must be nonnegative integers");
    };
    if min > max {
        return unknown("Length bounds are descending");
    }
    let size = match value {
        V::String(text) => text.encode_utf16().count(),
        V::List(values) => values.len(),
        _ => return verdict(false, "Length requires a string or list"),
    };
    verdict(
        size >= min && size <= max,
        "Value length is outside its declared bounds",
    )
}
fn whitespace(character: char) -> bool {
    matches!(character, '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}
pub(crate) fn format(terms: &[&str], value: &V) -> C {
    let [name] = terms else {
        return unknown("Format requires exactly one reviewed format name");
    };
    let V::String(text) = value else {
        return verdict(false, "Format requires a string");
    };
    let valid = match *name {
        "Email" => {
            let parts = text.split('@').collect::<Vec<_>>();
            if let [local, domain] = parts.as_slice() {
                !local.is_empty()
                    && !text.chars().any(whitespace)
                    && domain
                        .rsplit_once('.')
                        .is_some_and(|(left, right)| !left.is_empty() && !right.is_empty())
            } else {
                false
            }
        }
        "UUID" => {
            text.len() == 36
                && text.bytes().enumerate().all(|(index, byte)| match index {
                    8 | 13 | 18 | 23 => byte == b'-',
                    14 => (b'1'..=b'5').contains(&byte),
                    19 => matches!(byte.to_ascii_lowercase(), b'8' | b'9' | b'a' | b'b'),
                    _ => byte.is_ascii_hexdigit(),
                })
        }
        "Uri" => url::Url::parse(text).is_ok(),
        // Reject impossible calendar dates instead of the reference Date.parse
        // rollover. This intentional tightening is independent of the target.
        "Date" => {
            text.len() == 10
                && text.bytes().enumerate().all(|(i, b)| {
                    if i == 4 || i == 7 {
                        b == b'-'
                    } else {
                        b.is_ascii_digit()
                    }
                })
                && chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d").is_ok()
        }
        "DateTime" => chrono::DateTime::parse_from_rfc3339(text).is_ok(),
        _ => return unknown("Unsupported format name"),
    };
    verdict(valid, "Value does not satisfy the declared format")
}
