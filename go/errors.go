package ttid

import "fmt"

// TtidError is returned when constructing or validating a raw TTID.
type TtidError struct {
	Kind ErrorKind
	TypeID uint16
}

type ErrorKind uint8

const (
	ErrTimestampOutOfRange ErrorKind = iota + 1
	ErrInvalidUUID
	ErrUnknownTypeID
	ErrInvalidFormat
	ErrUnknownTypeName
	ErrInvalidShortUUID
	ErrTypeMismatch
	ErrMissingCodec
)

func (e TtidError) Error() string {
	switch e.Kind {
	case ErrTimestampOutOfRange:
		return "timestamp exceeds 48-bit TTID limit"
	case ErrInvalidUUID:
		return "uuid is not a valid TTID UUIDv8"
	case ErrUnknownTypeID:
		return fmt.Sprintf("uuid contains unknown type id for this IdType: %d", e.TypeID)
	case ErrInvalidFormat:
		return "invalid TTID string format, expected <type>_<shortuuid>"
	case ErrUnknownTypeName:
		return "unknown TTID type name"
	case ErrInvalidShortUUID:
		return "invalid shortuuid value"
	case ErrTypeMismatch:
		return "type name and encoded type id do not match"
	case ErrMissingCodec:
		return "ttid id is not bound to a codec"
	default:
		return "ttid error"
	}
}

func (e TtidError) Is(target error) bool {
	switch other := target.(type) {
	case TtidError:
		return e.Kind == other.Kind
	case *TtidError:
		return other != nil && e.Kind == other.Kind
	default:
		return false
	}
}

// ParseTtidError wraps failures while parsing <type-name>_<shortuuid> strings.
type ParseTtidError struct{ Err error }

func (e ParseTtidError) Error() string {
	if te, ok := e.Err.(TtidError); ok {
		switch te.Kind {
		case ErrInvalidFormat, ErrUnknownTypeName, ErrInvalidShortUUID, ErrTypeMismatch:
			return te.Error()
		default:
			return "invalid TTID payload: " + te.Error()
		}
	}
	return e.Err.Error()
}

func (e ParseTtidError) Unwrap() error { return e.Err }
