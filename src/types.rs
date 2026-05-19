#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Patch<T> {
    #[default]
    Absent,
    Set(T),
}

impl<T> Patch<T> {
    pub const fn is_absent(&self) -> bool {
        matches!(self, Patch::Absent)
    }
}

#[cfg(feature = "serde")]
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Patch<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        T::deserialize(d).map(Patch::Set)
    }
}

#[cfg(feature = "serde")]
impl<T: Serialize> Serialize for Patch<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Patch::Set(v) => v.serialize(s),
            Patch::Absent => s.serialize_none(),
        }
    }
}

impl<T> From<Option<T>> for Patch<T> {
    fn from(opt: Option<T>) -> Self {
        match opt {
            Some(v) => Patch::Set(v),
            None => Patch::Absent,
        }
    }
}

impl<T> From<Patch<T>> for Option<T> {
    fn from(patch: Patch<T>) -> Self {
        match patch {
            Patch::Set(v) => Some(v),
            Patch::Absent => None,
        }
    }
}

impl<T> From<T> for Patch<T> {
    fn from(value: T) -> Self {
        Patch::Set(value)
    }
}
