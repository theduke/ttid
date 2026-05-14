package ttid

import (
	"encoding/json"
	"strings"
	"testing"
)

type MyType int

const (
	User MyType = iota + 1
	Org
	Session
)

type MyDomain struct{}

func (MyDomain) TypeID(v MyType) uint16 {
	switch v {
	case User:
		return 1
	case Org:
		return 2
	case Session:
		return 777
	default:
		return 0
	}
}

func (MyDomain) FromTypeID(id uint16) (MyType, bool) {
	switch id {
	case 1:
		return User, true
	case 2:
		return Org, true
	case 777:
		return Session, true
	default:
		return 0, false
	}
}

func (MyDomain) TypeName(v MyType) string {
	switch v {
	case User:
		return "user"
	case Org:
		return "org"
	case Session:
		return "session"
	default:
		return "unknown"
	}
}

func (MyDomain) FromTypeName(name string) (MyType, bool) {
	switch name {
	case "user":
		return User, true
	case "org":
		return Org, true
	case "session":
		return Session, true
	default:
		return 0, false
	}
}

func TestRoundTrip(t *testing.T) {
	c := NewCodec[MyType](MyDomain{})
	id, err := c.FromParts(1_700_000_000_000, Session, 42)
	if err != nil {
		t.Fatal(err)
	}
	if got := id.TimestampMs(); got != 1_700_000_000_000 {
		t.Fatalf("timestamp = %d", got)
	}
	if got := id.TypeID(); got != 777 {
		t.Fatalf("type id = %d", got)
	}
	if got := id.Randomness(); got != 42 {
		t.Fatalf("randomness = %d", got)
	}
	if got := id.String(); !strings.HasPrefix(got, "session_") {
		t.Fatalf("string = %q", got)
	}
	parsed, err := c.Parse(id.String())
	if err != nil {
		t.Fatal(err)
	}
	if !parsed.Equal(id) {
		t.Fatal("parse roundtrip mismatch")
	}
}

func TestValidation(t *testing.T) {
	c := NewCodec[MyType](MyDomain{})
	if _, err := c.FromParts(timestampMax+1, User, 1); err == nil {
		t.Fatal("expected timestamp error")
	}
	if _, err := c.FromUUID(UUID{}); err == nil {
		t.Fatal("expected invalid uuid error")
	}
}

func TestUnknownTypeMismatch(t *testing.T) {
	c := NewCodec[MyType](MyDomain{})
	id, err := c.FromParts(1_700_000_000_000, User, 9)
	if err != nil {
		t.Fatal(err)
	}
	other := NewCodec[MyType](MyDomain{})
	if _, err := other.Parse("org_" + id.ShortUUID()); err == nil {
		t.Fatal("expected type mismatch")
	}
}

func TestJSONAndBinary(t *testing.T) {
	c := NewCodec[MyType](MyDomain{})
	id, err := c.New(User)
	if err != nil {
		t.Fatal(err)
	}
	data, err := json.Marshal(id)
	if err != nil {
		t.Fatal(err)
	}
	var got ID[MyType]
	got.SetCodec(c)
	if err := json.Unmarshal(data, &got); err != nil {
		t.Fatal(err)
	}
	if !got.Equal(id) {
		t.Fatal("json roundtrip mismatch")
	}
	bin, err := id.MarshalBinary()
	if err != nil {
		t.Fatal(err)
	}
	var raw ID[MyType]
	if err := raw.UnmarshalBinary(bin); err != nil {
		t.Fatal(err)
	}
	if !raw.Equal(id) {
		t.Fatal("binary roundtrip mismatch")
	}
}

func TestUUIDHelpers(t *testing.T) {
	u, err := NewUUID()
	if err != nil {
		t.Fatal(err)
	}
	if u.Version() != 4 || u.Variant() != 2 {
		t.Fatalf("unexpected entropy uuid layout: %v", u)
	}
	text := u.String()
	parsed, err := ParseUUID(text)
	if err != nil {
		t.Fatal(err)
	}
	if parsed != u {
		t.Fatal("uuid roundtrip mismatch")
	}
}
