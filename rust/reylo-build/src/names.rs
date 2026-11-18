//! The Rust names of what a query module writes: an enum's variants, from the values the database
//! stores, and a type's name, from the database type or the column it is written for.
//!
//! A variant is its value in PascalCase, `_`, `-` and spaces separating the words (`send_email` and
//! `send-email` are each `SendEmail`). A value that is no Rust name that way (`1st`, `a/b`, `Self`),
//! or that would share its name with another value of the enum, is spelled out after a leading `_`:
//! each ASCII letter and digit as it is, every other character as `_`, its code point in hex and `_`
//! again (`send_email` and `send-email`, together, are `_send_5f_email` and `_send_2d_email`). A
//! PascalCase name never starts with `_` and the spelling-out can be read back, so no two values
//! of an enum ever share a name, and each value's name depends on the values alone, never on their
//! order: a value added before another cannot take that one's name.

use std::collections::HashMap;

/// The variants of an enum whose values are `labels`, in the same order.
pub fn variants(labels: &[String]) -> Vec<String> {
    let candidates: Vec<Option<String>> = labels.iter().map(|l| pascal(l)).collect();
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for c in candidates.iter().flatten() {
        *counts.entry(c.as_str()).or_default() += 1;
    }
    labels
        .iter()
        .zip(&candidates)
        .map(|(label, candidate)| match candidate {
            Some(c) if counts[c.as_str()] == 1 => c.clone(),
            _ => spelled_out(label),
        })
        .collect()
}

/// A type's name from a database name or a column's: its PascalCase, else its spelling-out.
pub fn type_name(name: &str) -> String {
    pascal(name).unwrap_or_else(|| spelled_out(name))
}

/// `name` in PascalCase, where it is a Rust name that way: ASCII letters and digits in words that
/// `_`, `-` and spaces separate, starting with a letter, and not `Self`.
pub fn pascal(name: &str) -> Option<String> {
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ' ')) {
        return None;
    }
    let out: String = name.split(['_', '-', ' ']).filter(|w| !w.is_empty()).map(|w| w[..1].to_ascii_uppercase() + &w[1..]).collect();
    (out.starts_with(|c: char| c.is_ascii_alphabetic()) && out != "Self").then_some(out)
}

/// `_`, then each ASCII letter and digit of `name` as it is and every other character as `_`, its
/// code point in hex, and `_`; `__` for the empty name.
pub fn spelled_out(name: &str) -> String {
    if name.is_empty() {
        return "__".into();
    }
    let mut out = String::from("_");
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else {
            out.push_str(&format!("_{:x}_", c as u32));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| v.to_string()).collect()
    }

    #[test]
    fn a_value_is_its_pascal_case() {
        assert_eq!(variants(&labels(&["queued", "send_email", "resize-image", "in progress", "HTTP2"])), ["Queued", "SendEmail", "ResizeImage", "InProgress", "HTTP2"]);
    }

    #[test]
    fn a_value_that_is_no_name_is_spelled_out() {
        assert_eq!(variants(&labels(&["1st", "a/b", "Self", "", "é"])), ["_1st", "_a_2f_b", "_Self", "__", "__e9_"]);
    }

    #[test]
    fn values_that_share_a_name_are_each_spelled_out() {
        assert_eq!(variants(&labels(&["send_email", "send-email", "other"])), ["_send_5f_email", "_send_2d_email", "Other"]);
        assert_eq!(variants(&labels(&["admin", "Admin"])), ["_admin", "_Admin"]);
    }

    #[test]
    fn a_name_does_not_depend_on_the_order_of_the_values() {
        let forward = variants(&labels(&["send-email", "send_email", "x"]));
        let backward = variants(&labels(&["x", "send_email", "send-email"]));
        assert_eq!(forward[0], backward[2]);
        assert_eq!(forward[1], backward[1]);
        assert_eq!(forward[2], backward[0]);
    }

    #[test]
    fn no_two_values_share_a_name() {
        // Every value of up to three characters from an alphabet of lookalikes, in one enum.
        let alphabet = ['a', 'A', '_', '-', ' ', '1', '.', 'S'];
        let mut values = vec![String::new()];
        for len in 1..=3 {
            let mut stack = vec![String::new()];
            for _ in 0..len {
                stack = stack.into_iter().flat_map(|s| alphabet.iter().map(move |c| format!("{s}{c}"))).collect();
            }
            values.extend(stack);
        }
        let names = variants(&values);
        let mut seen = std::collections::HashSet::new();
        for (value, name) in values.iter().zip(&names) {
            assert!(syn::parse_str::<syn::Ident>(name).is_ok(), "{value:?} is {name:?}, no Rust name");
            assert!(seen.insert(name.clone()), "{value:?} shares the name {name:?}");
        }
    }
}
