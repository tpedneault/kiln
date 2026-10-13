//! Item IDs: `PREFIX-N`, written with at least three digits.
//!
//! Exactly one spelling per number (`REQ-007`, `REQ-014`, `REQ-1000`;
//! never `REQ-7` or `REQ-0014`), so an ID found by `grep` or typed in a
//! review is the ID in the file, and IDs sort the same as text and as
//! numbers up to 999. Three digits because most projects never pass 999
//! items of one type, and those that do simply get a fourth digit.

use std::fmt;
use std::str::FromStr;

/// An item ID. Ordered by prefix, then number.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemId {
    prefix: String,
    number: u32,
}

/// Why a string isn't an ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdError {
    /// Not `PREFIX-DIGITS` with an uppercase prefix.
    Shape,
    /// Zero isn't an ID number.
    Zero,
    /// A number written another way than its one spelling; carries it.
    Spelling(ItemId),
}

impl fmt::Display for IdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdError::Shape => write!(
                f,
                "an ID is PREFIX-NUMBER, the prefix uppercase letters and digits (REQ-014)"
            ),
            IdError::Zero => write!(f, "ID numbers start at 1"),
            IdError::Spelling(id) => {
                write!(f, "write it {id} (at least three digits, no extra zeros)")
            }
        }
    }
}

impl ItemId {
    /// `prefix` must already be valid ([`is_prefix`]); `number` ≥ 1.
    pub fn new(prefix: impl Into<String>, number: u32) -> Self {
        ItemId {
            prefix: prefix.into(),
            number,
        }
    }

    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    pub fn number(&self) -> u32 {
        self.number
    }
}

impl fmt::Display for ItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{:03}", self.prefix, self.number)
    }
}

impl FromStr for ItemId {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, IdError> {
        let (prefix, digits) = s.rsplit_once('-').ok_or(IdError::Shape)?;
        if !is_prefix(prefix) || digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Err(IdError::Shape);
        }
        // More digits than a u32 holds is a shape problem, not a number.
        let number: u32 = digits.parse().map_err(|_| IdError::Shape)?;
        if number == 0 {
            return Err(IdError::Zero);
        }
        let id = ItemId::new(prefix, number);
        if id.to_string() != s {
            return Err(IdError::Spelling(id));
        }
        Ok(id)
    }
}

/// Uppercase ASCII letters and digits, starting with a letter.
pub fn is_prefix(s: &str) -> bool {
    let mut bytes = s.bytes();
    matches!(bytes.next(), Some(b'A'..=b'Z'))
        && bytes.all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_ids_parse_and_print_the_same() {
        for s in ["REQ-001", "REQ-014", "ADR-999", "REQ-1000", "X2-123456"] {
            assert_eq!(s.parse::<ItemId>().unwrap().to_string(), s);
        }
    }

    #[test]
    fn other_spellings_name_the_right_one() {
        assert_eq!(
            "REQ-14".parse::<ItemId>(),
            Err(IdError::Spelling(ItemId::new("REQ", 14)))
        );
        assert_eq!(
            "REQ-0014".parse::<ItemId>(),
            Err(IdError::Spelling(ItemId::new("REQ", 14)))
        );
    }

    #[test]
    fn bad_shapes_are_refused() {
        for s in [
            "",
            "REQ",
            "REQ-",
            "req-001",
            "1REQ-001",
            "REQ-01a",
            "REQ_001",
            "REQ-99999999999",
        ] {
            assert_eq!(s.parse::<ItemId>(), Err(IdError::Shape), "{s}");
        }
        assert_eq!("REQ-000".parse::<ItemId>(), Err(IdError::Zero));
    }

    #[test]
    fn ids_order_by_prefix_then_number() {
        let mut ids: Vec<ItemId> = ["REQ-010", "ADR-002", "REQ-002", "REQ-1000"]
            .iter()
            .map(|s| s.parse().unwrap())
            .collect();
        ids.sort();
        let shown: Vec<String> = ids.iter().map(ToString::to_string).collect();
        assert_eq!(shown, ["ADR-002", "REQ-002", "REQ-010", "REQ-1000"]);
    }
}
