//! Shared tri-state for the writable fields of Tasks resources,
//! distinguishing a field left out of a PATCH body from one explicitly
//! cleared.
//!
//! <https://developers.google.com/workspace/tasks/reference/rest/v1/tasks/patch>

use core::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A writable Google Tasks resource field: absent from the serialized
/// body, explicitly null, or set to a value.
///
/// PATCH semantics make the three states meaningful: an omitted field
/// keeps its stored value, a null one is cleared, a present one is
/// replaced. Deserialization maps a JSON null onto `Null` and a missing
/// field onto `Unset` (via the serde `default`).
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub enum GtasksField<T> {
    /// Left out of the serialized body; a patch preserves the stored
    /// value.
    #[default]
    Unset,
    /// Serialized as an explicit null; a patch clears the stored value.
    Null,
    /// Serialized as the value itself.
    Set(T),
}

impl<T> GtasksField<T> {
    /// True for the variant skipped by serialization.
    pub fn is_unset(&self) -> bool {
        matches!(self, Self::Unset)
    }

    /// The set value, `None` for both unset and null.
    pub fn as_option(&self) -> Option<&T> {
        match self {
            Self::Set(value) => Some(value),
            _ => None,
        }
    }

    /// The set value by value, `None` for both unset and null.
    pub fn into_option(self) -> Option<T> {
        match self {
            Self::Set(value) => Some(value),
            _ => None,
        }
    }

    /// `Set` for `Some`, `Null` for `None`.
    ///
    /// This is the natural encoding of a full-state patch body, where
    /// every managed field is either replaced or cleared.
    pub fn set_or_null(value: Option<T>) -> Self {
        match value {
            Some(value) => Self::Set(value),
            None => Self::Null,
        }
    }
}

impl<T: Deref> GtasksField<T> {
    /// The set value dereferenced (e.g. `&str` out of a `String`
    /// field), `None` for both unset and null.
    pub fn as_deref(&self) -> Option<&T::Target> {
        self.as_option().map(|value| value.deref())
    }
}

impl<T> From<T> for GtasksField<T> {
    fn from(value: T) -> Self {
        Self::Set(value)
    }
}

impl<T: Serialize> Serialize for GtasksField<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            // NOTE: Unset only reaches here without the paired
            // skip_serializing_if attribute; null is the least wrong
            // encoding then.
            Self::Unset | Self::Null => serializer.serialize_none(),
            Self::Set(value) => value.serialize(serializer),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for GtasksField<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Option::<T>::deserialize(deserializer)? {
            Some(value) => Ok(Self::Set(value)),
            None => Ok(Self::Null),
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::String;

    use serde::{Deserialize, Serialize};
    use serde_json::json;

    use crate::v1::field::GtasksField;

    #[derive(Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
    struct Body {
        #[serde(default, skip_serializing_if = "GtasksField::is_unset")]
        notes: GtasksField<String>,
        #[serde(default, skip_serializing_if = "GtasksField::is_unset")]
        deleted: GtasksField<bool>,
    }

    #[test]
    fn unset_fields_are_omitted() {
        let body = serde_json::to_value(Body::default()).unwrap();
        assert_eq!(body, json!({}));
    }

    #[test]
    fn null_fields_serialize_as_explicit_null() {
        let body = Body {
            notes: GtasksField::Null,
            deleted: GtasksField::Null,
        };

        assert_eq!(
            serde_json::to_value(&body).unwrap(),
            json!({ "notes": null, "deleted": null })
        );
    }

    #[test]
    fn deserialization_distinguishes_missing_from_null() {
        let body: Body = serde_json::from_value(json!({ "notes": null })).unwrap();
        assert_eq!(body.notes, GtasksField::Null);
        assert_eq!(body.deleted, GtasksField::Unset);

        let body: Body = serde_json::from_value(json!({ "deleted": false })).unwrap();
        assert_eq!(body.notes, GtasksField::Unset);
        assert_eq!(body.deleted, GtasksField::Set(false));
    }
}
