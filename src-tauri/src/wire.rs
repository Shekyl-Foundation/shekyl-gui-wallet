// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Types that exist only to cross the Tauri edge correctly.
//!
//! The frontend is JavaScript: its `number` is an IEEE double and loses
//! integers above 2^53. A `u64` atomic amount serialized as a JSON number
//! therefore arrives rounded — and a rounded fee or amount defeats the send
//! flow's one invariant, that the fee the user confirms is the fee that ships.
//! The wallet contract (`wallet_rpc.yaml`) already names the answer:
//! `AtomicUnitsString`, a decimal string, parsed on the JS side with `BigInt`.
//!
//! Every atomic amount on the Tauri wire is this type, in both directions. A
//! JSON number is *rejected* on input rather than accepted leniently, because
//! a number that reached us has already been rounded by the sender.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use shekyl_units::AtomicUnits;

/// The contract's `AtomicUnitsString`: a `u64` of atomic units carried as a
/// decimal string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtomicUnitsString(u64);

impl AtomicUnitsString {
    pub fn to_raw(self) -> u64 {
        self.0
    }

    pub fn to_atomic_units(self) -> AtomicUnits {
        AtomicUnits::from_raw(self.0)
    }
}

impl From<u64> for AtomicUnitsString {
    fn from(raw: u64) -> Self {
        Self(raw)
    }
}

impl From<AtomicUnits> for AtomicUnitsString {
    fn from(units: AtomicUnits) -> Self {
        Self(units.to_raw())
    }
}

impl Serialize for AtomicUnitsString {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for AtomicUnitsString {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // `String`, not `&str`: this must also deserialize from an owned
        // `serde_json::Value`, which cannot lend a borrowed string.
        let text = String::deserialize(deserializer)?;
        text.parse::<u64>().map(Self).map_err(|_| {
            serde::de::Error::custom(format!(
                "expected a decimal string of atomic units, got {text:?}"
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first integer a JS `number` cannot hold.
    const BEYOND_DOUBLE: u64 = (1u64 << 53) + 1;

    #[test]
    fn serializes_as_a_decimal_string_not_a_number() {
        let v = serde_json::to_value(AtomicUnitsString::from(BEYOND_DOUBLE)).unwrap();
        assert_eq!(v, serde_json::Value::String("9007199254740993".into()));
        assert_eq!(
            serde_json::to_string(&AtomicUnitsString::from(u64::MAX)).unwrap(),
            "\"18446744073709551615\""
        );
    }

    #[test]
    fn deserializes_the_decimal_string_losslessly() {
        let parsed: AtomicUnitsString = serde_json::from_str("\"9007199254740993\"").unwrap();
        assert_eq!(parsed.to_raw(), BEYOND_DOUBLE);
        let owned: AtomicUnitsString =
            serde_json::from_value(serde_json::json!("9007199254740993")).unwrap();
        assert_eq!(owned, parsed);
        assert_eq!(
            parsed.to_atomic_units(),
            AtomicUnits::from_raw(BEYOND_DOUBLE)
        );
    }

    #[test]
    fn rejects_a_json_number_and_anything_not_a_u64_decimal() {
        for bad in [
            "9007199254740993",
            "\"1.5\"",
            "\"-1\"",
            "\"\"",
            "\"1e9\"",
            "\"18446744073709551616\"",
            "null",
        ] {
            assert!(
                serde_json::from_str::<AtomicUnitsString>(bad).is_err(),
                "accepted {bad}"
            );
        }
    }
}
