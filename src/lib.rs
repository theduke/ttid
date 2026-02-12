//! Typed, readable IDs on top of UUIDv8.
//!
//! TTID combines:
//! - a `48-bit` millisecond timestamp,
//! - a `16-bit` user-defined type id,
//! - `58-bit` randomness,
//! - into a UUIDv8-compatible bit layout.
//!
//! The human-facing string format is:
//! `"<type-name>_<shortuuid>"`
//!
//! where `shortuuid` is encoded with the [`short-uuid`](https://crates.io/crates/short-uuid)
//! crate's default base58 alphabet.
//!
//! # Motivation
//!
//! Plain UUIDs are globally unique but not ergonomic in logs and APIs.
//! TTIDs keep UUID interoperability while adding:
//! - explicit type context,
//! - compact text form,
//! - embedded time for quick debugging and coarse ordering.
//!
//! The binary packing is **timestamp-first** (most significant payload bits),
//! which improves locality for UUID-sorted database indexes.
//!
//! # Example
//!
//! ```
//! use std::str::FromStr;
//! use ttid::{IdType, Ttid};
//!
//! #[derive(Clone, Copy, Debug, PartialEq, Eq)]
//! enum MyType {
//!     User,
//!     Session,
//! }
//!
//! impl IdType for MyType {
//!     fn to_type_id(self) -> u16 {
//!         match self {
//!             Self::User => 1,
//!             Self::Session => 2,
//!         }
//!     }
//!
//!     fn from_type_id(id: u16) -> Option<Self> {
//!         match id {
//!             1 => Some(Self::User),
//!             2 => Some(Self::Session),
//!             _ => None,
//!         }
//!     }
//!
//!     fn as_type_name(self) -> &'static str {
//!         match self {
//!             Self::User => "user",
//!             Self::Session => "session",
//!         }
//!     }
//!
//!     fn from_type_name(name: &str) -> Option<Self> {
//!         match name {
//!             "user" => Some(Self::User),
//!             "session" => Some(Self::Session),
//!             _ => None,
//!         }
//!     }
//! }
//!
//! let id = Ttid::<MyType>::new(MyType::User).unwrap();
//! let text = id.to_string();
//! let parsed = Ttid::<MyType>::from_str(&text).unwrap();
//!
//! assert_eq!(parsed, id);
//! assert_eq!(parsed.id_type(), MyType::User);
//! ```

use std::fmt;
use std::marker::PhantomData;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use short_uuid::ShortUuid;
use uuid::Uuid;

mod deser;
mod error;
use deser::{
    RANDOM_BITS, RANDOM_MASK, TIMESTAMP_MAX, TYPE_BITS, TYPE_ID_MAX, decode_payload_from_uuid,
    encode_payload_to_uuid,
};
pub use error::{ParseTtidError, TtidError};

/// Maps a Rust type enum to a compact numeric id and readable type name.
///
/// The mapping must be stable for persisted data.
///
/// - `to_type_id` / `from_type_id` map to the packed `16-bit` type field.
/// - `as_type_name` / `from_type_name` map to the string prefix in
///   `<type-name>_<shortuuid>`.
pub trait IdType: Sized + Copy {
    /// Convert enum value to numeric type id.
    fn to_type_id(self) -> u16;

    /// Convert numeric type id back to enum.
    fn from_type_id(id: u16) -> Option<Self>;

    /// Convert enum value to stable human-readable name.
    fn as_type_name(self) -> &'static str;

    /// Parse type name back to enum.
    fn from_type_name(name: &str) -> Option<Self>;
}

/// Typed TTID wrapper around `uuid::Uuid`.
///
/// `T` is the type-domain enum implementing [`IdType`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ttid<T: IdType> {
    uuid: Uuid,
    marker: PhantomData<T>,
}

impl<T: IdType> Ttid<T> {
    /// Create a new TTID from current Unix timestamp in milliseconds,
    /// `ty`, and 58 random bits derived from UUIDv4 randomness.
    pub fn new(ty: T) -> Result<Self, TtidError> {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_millis() as u64;
        let random_bits = Uuid::new_v4().as_u128() as u64 & RANDOM_MASK;

        Self::from_parts(now_ms, ty, random_bits)
    }

    /// Construct from explicit components.
    ///
    /// `randomness` values larger than 58 bits are masked to the low 58 bits.
    pub fn from_parts(timestamp_ms: u64, ty: T, randomness: u64) -> Result<Self, TtidError> {
        let type_id = ty.to_type_id();

        if timestamp_ms > TIMESTAMP_MAX {
            return Err(TtidError::TimestampOutOfRange);
        }

        let payload = ((timestamp_ms as u128) << (TYPE_BITS + RANDOM_BITS))
            | ((type_id as u128) << RANDOM_BITS)
            | ((randomness & RANDOM_MASK) as u128);

        let uuid = encode_payload_to_uuid(payload);
        Ok(Self {
            uuid,
            marker: PhantomData,
        })
    }

    /// Validate and wrap a UUID as TTID.
    pub fn from_uuid(uuid: Uuid) -> Result<Self, TtidError> {
        let payload = decode_payload_from_uuid(uuid).ok_or(TtidError::InvalidUuid)?;
        let type_id = ((payload >> RANDOM_BITS) & (TYPE_ID_MAX as u128)) as u16;

        if T::from_type_id(type_id).is_none() {
            return Err(TtidError::UnknownTypeId(type_id));
        }

        Ok(Self {
            uuid,
            marker: PhantomData,
        })
    }

    /// Borrow the raw UUID value.
    pub fn as_uuid(&self) -> Uuid {
        self.uuid
    }

    /// Extract millisecond Unix timestamp.
    pub fn timestamp_ms(&self) -> u64 {
        let payload = decode_payload_from_uuid(self.uuid).expect("internal TTID is always valid");
        (payload >> (TYPE_BITS + RANDOM_BITS)) as u64
    }

    /// Extract numeric type id.
    pub fn type_id(&self) -> u16 {
        let payload = decode_payload_from_uuid(self.uuid).expect("internal TTID is always valid");
        ((payload >> RANDOM_BITS) & (TYPE_ID_MAX as u128)) as u16
    }

    /// Extract typed enum variant.
    pub fn id_type(&self) -> T {
        T::from_type_id(self.type_id()).expect("type id validated at construction")
    }

    /// Extract random 58-bit component.
    pub fn randomness(&self) -> u64 {
        let payload = decode_payload_from_uuid(self.uuid).expect("internal TTID is always valid");
        (payload as u64) & RANDOM_MASK
    }

    /// Return shortuuid encoding of the underlying UUID.
    pub fn short_uuid(&self) -> ShortUuid {
        ShortUuid::from_uuid(&self.uuid)
    }
}

impl<T: IdType> fmt::Display for Ttid<T> {
    /// Formats as `<type-name>_<shortuuid>`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ty = self.id_type();
        write!(f, "{}_{}", ty.as_type_name(), self.short_uuid())
    }
}

impl<T: IdType> FromStr for Ttid<T> {
    type Err = ParseTtidError;

    /// Parses `<type-name>_<shortuuid>`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (type_name, short) = s.split_once('_').ok_or(ParseTtidError::InvalidFormat)?;

        let parsed_type = T::from_type_name(type_name).ok_or(ParseTtidError::UnknownTypeName)?;
        let short = ShortUuid::parse_str(short).map_err(|_| ParseTtidError::InvalidShortUuid)?;
        let uuid = short.to_uuid();

        let ttid = Ttid::<T>::from_uuid(uuid)?;
        if ttid.id_type().to_type_id() != parsed_type.to_type_id() {
            return Err(ParseTtidError::TypeMismatch);
        }

        Ok(ttid)
    }
}

impl<T: IdType> TryFrom<Uuid> for Ttid<T> {
    type Error = TtidError;

    fn try_from(value: Uuid) -> Result<Self, Self::Error> {
        Self::from_uuid(value)
    }
}

impl<T: IdType> From<Ttid<T>> for Uuid {
    fn from(value: Ttid<T>) -> Self {
        value.uuid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum MyType {
        User,
        Org,
        Session,
        Max,
    }

    impl IdType for MyType {
        fn to_type_id(self) -> u16 {
            match self {
                Self::User => 1,
                Self::Org => 2,
                Self::Session => 777,
                Self::Max => TYPE_ID_MAX,
            }
        }

        fn from_type_id(id: u16) -> Option<Self> {
            match id {
                1 => Some(Self::User),
                2 => Some(Self::Org),
                777 => Some(Self::Session),
                TYPE_ID_MAX => Some(Self::Max),
                _ => None,
            }
        }

        fn as_type_name(self) -> &'static str {
            match self {
                Self::User => "user",
                Self::Org => "org",
                Self::Session => "session",
                Self::Max => "max",
            }
        }

        fn from_type_name(name: &str) -> Option<Self> {
            match name {
                "user" => Some(Self::User),
                "org" => Some(Self::Org),
                "session" => Some(Self::Session),
                "max" => Some(Self::Max),
                _ => None,
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum NarrowType {
        User,
    }

    impl IdType for NarrowType {
        fn to_type_id(self) -> u16 {
            match self {
                Self::User => 1,
            }
        }

        fn from_type_id(id: u16) -> Option<Self> {
            match id {
                1 => Some(Self::User),
                _ => None,
            }
        }

        fn as_type_name(self) -> &'static str {
            "user"
        }

        fn from_type_name(name: &str) -> Option<Self> {
            match name {
                "user" => Some(Self::User),
                _ => None,
            }
        }
    }

    #[test]
    fn roundtrip_parts() {
        let ts = 1_735_689_010_123u64;
        let rand = 0x0abc_def1_2345_6789u64 & RANDOM_MASK;
        let ttid = Ttid::<MyType>::from_parts(ts, MyType::Session, rand).unwrap();

        assert_eq!(ttid.timestamp_ms(), ts);
        assert_eq!(ttid.type_id(), 777);
        assert_eq!(ttid.id_type(), MyType::Session);
        assert_eq!(ttid.randomness(), rand);

        let uuid = ttid.as_uuid();
        let parsed = Ttid::<MyType>::from_uuid(uuid).unwrap();
        assert_eq!(parsed, ttid);
    }

    #[test]
    fn accepts_max_timestamp_and_max_type() {
        let ttid = Ttid::<MyType>::from_parts(TIMESTAMP_MAX, MyType::Max, RANDOM_MASK).unwrap();

        assert_eq!(ttid.timestamp_ms(), TIMESTAMP_MAX);
        assert_eq!(ttid.type_id(), TYPE_ID_MAX);
        assert_eq!(ttid.randomness(), RANDOM_MASK);
    }

    #[test]
    fn new_uses_current_time() {
        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let ttid = Ttid::<MyType>::new(MyType::User).unwrap();

        let after = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        assert!(ttid.timestamp_ms() >= before);
        assert!(ttid.timestamp_ms() <= after);
    }

    #[test]
    fn display_and_parse_roundtrip() {
        let ttid = Ttid::<MyType>::from_parts(1_700_000_000_000, MyType::User, 42).unwrap();
        let rendered = ttid.to_string();

        assert!(rendered.starts_with("user_"));

        let parsed: Ttid<MyType> = rendered.parse().unwrap();
        assert_eq!(parsed, ttid);
    }

    #[test]
    fn parse_rejects_missing_separator() {
        let err = "user".parse::<Ttid<MyType>>().unwrap_err();
        assert!(matches!(err, ParseTtidError::InvalidFormat));
    }

    #[test]
    fn parse_rejects_unknown_type_name() {
        let uuid = Uuid::new_v4();
        let s = format!("does_not_exist_{}", ShortUuid::from_uuid(&uuid));

        let err = s.parse::<Ttid<MyType>>().unwrap_err();
        assert!(matches!(err, ParseTtidError::UnknownTypeName));
    }

    #[test]
    fn parse_rejects_invalid_short_uuid() {
        let err = "user_not-a-short-uuid".parse::<Ttid<MyType>>().unwrap_err();
        assert!(matches!(err, ParseTtidError::InvalidShortUuid));
    }

    #[test]
    fn detect_type_mismatch() {
        let ttid = Ttid::<MyType>::from_parts(1_700_000_000_000, MyType::User, 42).unwrap();
        let text = ttid.to_string();
        let wrong = text.replacen("user_", "org_", 1);

        let err = wrong.parse::<Ttid<MyType>>().unwrap_err();
        assert!(matches!(err, ParseTtidError::TypeMismatch));
    }

    #[test]
    fn reject_non_ttid_uuid() {
        let uuid = Uuid::new_v4();
        let err = Ttid::<MyType>::from_uuid(uuid).unwrap_err();
        assert!(matches!(err, TtidError::InvalidUuid));
    }

    #[test]
    fn reject_unknown_type_id_for_target_domain() {
        let session = Ttid::<MyType>::from_parts(1_700_000_000_000, MyType::Session, 9).unwrap();
        let err = Ttid::<NarrowType>::from_uuid(session.as_uuid()).unwrap_err();
        assert!(matches!(err, TtidError::UnknownTypeId(777)));
    }

    #[test]
    fn validates_part_limits() {
        let too_large_ts = TIMESTAMP_MAX + 1;
        let err = Ttid::<MyType>::from_parts(too_large_ts, MyType::User, 1).unwrap_err();
        assert!(matches!(err, TtidError::TimestampOutOfRange));

        let ttid = Ttid::<MyType>::from_parts(123, MyType::User, u64::MAX).unwrap();
        assert_eq!(ttid.randomness(), RANDOM_MASK);
    }

    #[test]
    fn uuid_version_and_variant_are_set() {
        let ttid = Ttid::<MyType>::from_parts(1_700_000_000_000, MyType::Org, 12345).unwrap();
        let bytes = *ttid.as_uuid().as_bytes();

        assert_eq!(bytes[6] >> 4, 0b1000);
        assert_eq!(bytes[8] & 0b1100_0000, 0b1000_0000);
    }

    #[test]
    fn uuid_and_ttid_conversion_traits_work() {
        let ttid = Ttid::<MyType>::from_parts(1_700_000_000_000, MyType::Org, 55).unwrap();

        let uuid: Uuid = ttid.into();
        let parsed = Ttid::<MyType>::try_from(uuid).unwrap();

        assert_eq!(parsed.id_type(), MyType::Org);
        assert_eq!(parsed.timestamp_ms(), 1_700_000_000_000);
    }

    #[test]
    fn two_new_ids_are_distinct() {
        let a = Ttid::<MyType>::new(MyType::User).unwrap();
        let b = Ttid::<MyType>::new(MyType::User).unwrap();

        assert_ne!(a, b);
    }

    #[test]
    fn timestamp_first_packing_improves_uuid_sorting() {
        let a = Ttid::<MyType>::from_parts(1_700_000_000_000, MyType::User, 0).unwrap();
        let b = Ttid::<MyType>::from_parts(1_700_000_000_001, MyType::User, 0).unwrap();
        let c = Ttid::<MyType>::from_parts(1_700_000_000_002, MyType::User, 0).unwrap();

        assert!(a.as_uuid().as_bytes() < b.as_uuid().as_bytes());
        assert!(b.as_uuid().as_bytes() < c.as_uuid().as_bytes());
    }

    #[test]
    fn ordering_within_same_timestamp_uses_type_then_randomness() {
        let ts = 1_700_000_000_000;
        let user_low = Ttid::<MyType>::from_parts(ts, MyType::User, 1).unwrap();
        let org_low = Ttid::<MyType>::from_parts(ts, MyType::Org, 1).unwrap();
        let org_high = Ttid::<MyType>::from_parts(ts, MyType::Org, 2).unwrap();

        assert!(user_low.as_uuid().as_bytes() < org_low.as_uuid().as_bytes());
        assert!(org_low.as_uuid().as_bytes() < org_high.as_uuid().as_bytes());
    }
}
