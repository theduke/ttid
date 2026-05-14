use std::fmt;

/// Errors returned when constructing or decoding raw TTID values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TtidError {
    /// Timestamp is larger than `TIMESTAMP_MAX`.
    TimestampOutOfRange,
    /// UUID doesn't match the TTID UUIDv8 invariants.
    InvalidUuid,
    /// Type id decoded from UUID is not known by `T`.
    UnknownTypeId(u16),
}

impl fmt::Display for TtidError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TimestampOutOfRange => f.write_str("timestamp exceeds 48-bit TTID limit"),
            Self::InvalidUuid => f.write_str("uuid is not a valid TTID UUIDv8"),
            Self::UnknownTypeId(type_id) => {
                write!(
                    f,
                    "uuid contains unknown type id for this IdType: {type_id}"
                )
            }
        }
    }
}

impl std::error::Error for TtidError {}

/// Errors returned when parsing `<type-name>_<shortuuid>` strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseTtidError {
    /// Input is not in `<type-name>_<shortuuid>` format.
    InvalidFormat,
    /// Type name cannot be resolved by `IdType::from_type_name`.
    UnknownTypeName,
    /// `shortuuid` part is invalid.
    InvalidShortUuid,
    /// Underlying TTID decoding error.
    Ttid(TtidError),
    /// Type name prefix and encoded type id disagree.
    TypeMismatch,
}

impl fmt::Display for ParseTtidError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat => {
                f.write_str("invalid TTID string format, expected <type>_<shortuuid>")
            }
            Self::UnknownTypeName => f.write_str("unknown TTID type name"),
            Self::InvalidShortUuid => f.write_str("invalid shortuuid value"),
            Self::Ttid(err) => write!(f, "invalid TTID payload: {err}"),
            Self::TypeMismatch => f.write_str("type name and encoded type id do not match"),
        }
    }
}

impl std::error::Error for ParseTtidError {}

impl From<TtidError> for ParseTtidError {
    fn from(value: TtidError) -> Self {
        Self::Ttid(value)
    }
}
