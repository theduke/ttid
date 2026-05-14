package ttid

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
)

// Domain maps a typed value to a stable numeric id and human-readable name.
//
// Implementations must be stable for persisted data.
type Domain[T comparable] interface {
	TypeID(T) uint16
	FromTypeID(uint16) (T, bool)
	TypeName(T) string
	FromTypeName(string) (T, bool)
}

// Codec binds a Domain to TTID construction and parsing.
type Codec[T comparable] struct {
	Domain Domain[T]
}

func NewCodec[T comparable](domain Domain[T]) *Codec[T] {
	return &Codec[T]{Domain: domain}
}

// ID is a typed TTID value.
//
// IDs are primarily identified by their UUID bytes. When bound to a Codec,
// they also support canonical formatting and parsing.
type ID[T comparable] struct {
	uuid  UUID
	codec *Codec[T]
}

// Bind returns a copy of id associated with c.
func (id ID[T]) Bind(c *Codec[T]) ID[T] {
	id.codec = c
	return id
}

// SetCodec binds id in place.
func (id *ID[T]) SetCodec(c *Codec[T]) { id.codec = c }

// UUID returns the underlying UUID bytes.
func (id ID[T]) UUID() UUID { return id.uuid }

// Equal reports whether two IDs refer to the same underlying UUID.
func (id ID[T]) Equal(other ID[T]) bool { return id.uuid == other.uuid }

// ShortUUID returns the canonical shortuuid encoding of the UUID.
func (id ID[T]) ShortUUID() string { return id.uuid.ShortUUID() }

// String returns the canonical TTID string when bound to a codec.
// Unbound IDs fall back to the raw UUID string.
func (id ID[T]) String() string {
	if id.codec == nil || id.codec.Domain == nil {
		return id.uuid.String()
	}
	if ty, err := id.codec.Type(id); err == nil {
		return fmt.Sprintf("%s_%s", id.codec.Domain.TypeName(ty), id.ShortUUID())
	}
	return id.uuid.String()
}

// TimestampMs returns the embedded Unix millisecond timestamp.
func (id ID[T]) TimestampMs() uint64 {
	payload, ok := decodePayloadFromUUID(id.uuid)
	if !ok {
		return 0
	}
	ts, _, _ := splitPayload(payload)
	return ts
}

// TypeID returns the embedded numeric type id.
func (id ID[T]) TypeID() uint16 {
	payload, ok := decodePayloadFromUUID(id.uuid)
	if !ok {
		return 0
	}
	_, typ, _ := splitPayload(payload)
	return typ
}

// Randomness returns the embedded random component.
func (id ID[T]) Randomness() uint64 {
	payload, ok := decodePayloadFromUUID(id.uuid)
	if !ok {
		return 0
	}
	_, _, r := splitPayload(payload)
	return r
}

// Type resolves the typed enum value using the bound codec.
func (id ID[T]) Type() (T, error) {
	var zero T
	if id.codec == nil || id.codec.Domain == nil {
		return zero, TtidError{Kind: ErrMissingCodec}
	}
	ty, ok := id.codec.Domain.FromTypeID(id.TypeID())
	if !ok {
		return zero, TtidError{Kind: ErrUnknownTypeID, TypeID: id.TypeID()}
	}
	return ty, nil
}

// MarshalBinary returns the raw 16-byte UUID.
func (id ID[T]) MarshalBinary() ([]byte, error) {
	b := make([]byte, len(id.uuid))
	copy(b, id.uuid[:])
	return b, nil
}

// UnmarshalBinary loads the raw UUID bytes.
func (id *ID[T]) UnmarshalBinary(data []byte) error {
	if len(data) != 16 {
		return errors.New("uuid must be 16 bytes")
	}
	copy(id.uuid[:], data)
	return nil
}

// MarshalText returns the canonical TTID string when bound to a codec.
func (id ID[T]) MarshalText() ([]byte, error) {
	if id.codec == nil || id.codec.Domain == nil {
		return []byte(id.uuid.String()), nil
	}
	return []byte(id.String()), nil
}

// UnmarshalText parses either a canonical TTID string or a raw UUID string.
func (id *ID[T]) UnmarshalText(data []byte) error {
	s := string(bytes.TrimSpace(data))
	if s == "" {
		return errors.New("empty value")
	}
	if bytes.ContainsRune(data, '_') {
		if id.codec == nil || id.codec.Domain == nil {
			return TtidError{Kind: ErrMissingCodec}
		}
		parsed, err := id.codec.Parse(s)
		if err != nil {
			return err
		}
		*id = parsed
		return nil
	}
	parsed, err := ParseUUID(s)
	if err != nil {
		return err
	}
	id.uuid = parsed
	return nil
}

// MarshalJSON serializes the canonical string.
func (id ID[T]) MarshalJSON() ([]byte, error) {
	return json.Marshal(id.String())
}

// UnmarshalJSON parses a JSON string.
func (id *ID[T]) UnmarshalJSON(data []byte) error {
	var s string
	if err := json.Unmarshal(data, &s); err != nil {
		return err
	}
	return id.UnmarshalText([]byte(s))
}

// New creates a fresh TTID using the current time and crypto/rand entropy.
func (c *Codec[T]) New(ty T) (ID[T], error) {
	r, err := makeRandom58()
	if err != nil {
		return ID[T]{}, err
	}
	return c.FromParts(nowUnixMilli(), ty, r)
}

// FromParts creates a TTID from explicit components.
func (c *Codec[T]) FromParts(timestampMs uint64, ty T, randomness uint64) (ID[T], error) {
	if c == nil || c.Domain == nil {
		return ID[T]{}, TtidError{Kind: ErrMissingCodec}
	}
	typeID := c.Domain.TypeID(ty)
	if rt, ok := c.Domain.FromTypeID(typeID); !ok || rt != ty {
		return ID[T]{}, TtidError{Kind: ErrUnknownTypeID, TypeID: typeID}
	}
	payload, err := buildPayload(timestampMs, typeID, randomness)
	if err != nil {
		return ID[T]{}, err
	}
	return ID[T]{uuid: encodePayloadToUUID(payload), codec: c}, nil
}

// FromUUID validates and wraps an existing UUID as a TTID.
func (c *Codec[T]) FromUUID(uuid UUID) (ID[T], error) {
	if c == nil || c.Domain == nil {
		return ID[T]{}, TtidError{Kind: ErrMissingCodec}
	}
	payload, ok := decodePayloadFromUUID(uuid)
	if !ok {
		return ID[T]{}, TtidError{Kind: ErrInvalidUUID}
	}
	_, typeID, _ := splitPayload(payload)
	if _, ok := c.Domain.FromTypeID(typeID); !ok {
		return ID[T]{}, TtidError{Kind: ErrUnknownTypeID, TypeID: typeID}
	}
	return ID[T]{uuid: uuid, codec: c}, nil
}

// Parse parses the canonical <type-name>_<shortuuid> format.
func (c *Codec[T]) Parse(s string) (ID[T], error) {
	if c == nil || c.Domain == nil {
		return ID[T]{}, TtidError{Kind: ErrMissingCodec}
	}
	sep := strings.IndexByte(s, '_')
	if sep <= 0 || sep == len(s)-1 {
		return ID[T]{}, ParseTtidError{Err: TtidError{Kind: ErrInvalidFormat}}
	}
	typeName, short := s[:sep], s[sep+1:]
	ty, ok := c.Domain.FromTypeName(typeName)
	if !ok {
		return ID[T]{}, ParseTtidError{Err: TtidError{Kind: ErrUnknownTypeName}}
	}
	uuid, err := decodeShortUUID(short)
	if err != nil {
		return ID[T]{}, ParseTtidError{Err: TtidError{Kind: ErrInvalidShortUUID}}
	}
	id, err := c.FromUUID(uuid)
	if err != nil {
		return ID[T]{}, ParseTtidError{Err: err}
	}
	if idTy, err := c.Type(id); err != nil || idTy != ty {
		return ID[T]{}, ParseTtidError{Err: TtidError{Kind: ErrTypeMismatch}}
	}
	return id, nil
}

// Type resolves the typed enum value for id.
func (c *Codec[T]) Type(id ID[T]) (T, error) {
	var zero T
	if c == nil || c.Domain == nil {
		return zero, TtidError{Kind: ErrMissingCodec}
	}
	ty, ok := c.Domain.FromTypeID(id.TypeID())
	if !ok {
		return zero, TtidError{Kind: ErrUnknownTypeID, TypeID: id.TypeID()}
	}
	return ty, nil
}

// String renders id in canonical form.
func (c *Codec[T]) String(id ID[T]) string {
	return id.Bind(c).String()
}
