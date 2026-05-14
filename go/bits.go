package ttid

import (
	"crypto/rand"
	"encoding/hex"
	"errors"
	"math/big"
	"strings"
	"time"
)

const (
	timestampBits  = 48
	typeBits       = 16
	randomBits     = 58
	payloadBits    = timestampBits + typeBits + randomBits
	timestampMax   = (uint64(1) << timestampBits) - 1
	typeIDMax      = ^uint16(0)
	randomnessMask = (uint64(1) << randomBits) - 1
	shortUUIDLen   = 22
)

var shortUUIDAlphabet = []byte("123456789abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ")
var shortUUIDDecodeMap [256]int16

func init() {
	for i := range shortUUIDDecodeMap {
		shortUUIDDecodeMap[i] = -1
	}
	for i, b := range shortUUIDAlphabet {
		shortUUIDDecodeMap[b] = int16(i)
	}
}

// UUID is a standard 16-byte UUID value.
type UUID [16]byte

func NewUUID() (UUID, error) {
	var u UUID
	if _, err := rand.Read(u[:]); err != nil {
		return UUID{}, err
	}
	// RFC4122 v4 layout for entropy source.
	u[6] = (u[6] & 0x0f) | 0x40
	u[8] = (u[8] & 0x3f) | 0x80
	return u, nil
}

func ParseUUID(s string) (UUID, error) {
	var u UUID
	parts := strings.Split(s, "-")
	if len(parts) != 5 || len(parts[0]) != 8 || len(parts[1]) != 4 || len(parts[2]) != 4 || len(parts[3]) != 4 || len(parts[4]) != 12 {
		return UUID{}, errors.New("invalid UUID string")
	}
	joined := strings.Join(parts, "")
	b, err := hex.DecodeString(joined)
	if err != nil || len(b) != 16 {
		return UUID{}, errors.New("invalid UUID string")
	}
	copy(u[:], b)
	return u, nil
}

func (u UUID) Bytes() [16]byte { return u }

func (u UUID) String() string {
	return hex.EncodeToString(u[0:4]) + "-" + hex.EncodeToString(u[4:6]) + "-" +
		hex.EncodeToString(u[6:8]) + "-" + hex.EncodeToString(u[8:10]) + "-" +
		hex.EncodeToString(u[10:16])
}

func (u UUID) Version() byte { return u[6] >> 4 }

func (u UUID) Variant() byte { return u[8] >> 6 }

func (u UUID) IsZero() bool { return u == UUID{} }

func (u UUID) ShortUUID() string { return encodeShortUUID(u) }

func (u UUID) MarshalText() ([]byte, error) { return []byte(u.String()), nil }

func (u *UUID) UnmarshalText(data []byte) error {
	parsed, err := ParseUUID(string(data))
	if err != nil {
		return err
	}
	*u = parsed
	return nil
}

func nowUnixMilli() uint64 { return uint64(time.Now().UnixMilli()) }

func makeRandom58() (uint64, error) {
	var buf [8]byte
	if _, err := rand.Read(buf[:]); err != nil {
		return 0, err
	}
	v := uint64(buf[0])<<56 | uint64(buf[1])<<48 | uint64(buf[2])<<40 | uint64(buf[3])<<32 |
		uint64(buf[4])<<24 | uint64(buf[5])<<16 | uint64(buf[6])<<8 | uint64(buf[7])
	return v & randomnessMask, nil
}

func buildPayload(timestampMs uint64, typeID uint16, randomness uint64) (*big.Int, error) {
	if timestampMs > timestampMax {
		return nil, TtidError{Kind: ErrTimestampOutOfRange}
	}
	payload := new(big.Int).SetUint64(timestampMs)
	payload.Lsh(payload, typeBits)
	payload.Or(payload, new(big.Int).SetUint64(uint64(typeID)))
	payload.Lsh(payload, randomBits)
	payload.Or(payload, new(big.Int).SetUint64(randomness&randomnessMask))
	return payload, nil
}

func splitPayload(payload *big.Int) (timestampMs uint64, typeID uint16, randomness uint64) {
	if payload == nil {
		return 0, 0, 0
	}
	p := new(big.Int).Set(payload)
	randomness = p.Uint64() & randomnessMask
	p.Rsh(p, randomBits)
	typeID = uint16(p.Uint64() & uint64(typeIDMax))
	p.Rsh(p, typeBits)
	timestampMs = p.Uint64()
	return
}

func encodePayloadToUUID(payload *big.Int) UUID {
	var u UUID
	bitIdx := payloadBits - 1
	for uuidBitPos := 127; uuidBitPos >= 0; uuidBitPos-- {
		if isFixedUUIDBit(uuidBitPos) {
			continue
		}
		if payload.Bit(bitIdx) == 1 {
			setBit(&u, uuidBitPos, 1)
		}
		bitIdx--
	}
	setBit(&u, 79, 1)
	setBit(&u, 78, 0)
	setBit(&u, 77, 0)
	setBit(&u, 76, 0)
	setBit(&u, 63, 1)
	setBit(&u, 62, 0)
	return u
}

func decodePayloadFromUUID(u UUID) (*big.Int, bool) {
	if !isValidTTIDUUID(u) {
		return nil, false
	}
	payload := new(big.Int)
	for uuidBitPos := 127; uuidBitPos >= 0; uuidBitPos-- {
		if isFixedUUIDBit(uuidBitPos) {
			continue
		}
		payload.Lsh(payload, 1)
		if getBit(u, uuidBitPos) == 1 {
			payload.Or(payload, big.NewInt(1))
		}
	}
	return payload, true
}

func isValidTTIDUUID(u UUID) bool {
	return u.Version() == 0b1000 && u.Variant() == 0b10
}

func isFixedUUIDBit(bitPos int) bool {
	return bitPos == 79 || bitPos == 78 || bitPos == 77 || bitPos == 76 || bitPos == 63 || bitPos == 62
}

func setBit(u *UUID, bitPos int, value uint8) {
	byteIdx := 15 - (bitPos / 8)
	bitIdx := uint(bitPos % 8)
	mask := byte(1 << bitIdx)
	if value == 0 {
		u[byteIdx] &^= mask
		return
	}
	u[byteIdx] |= mask
}

func getBit(u UUID, bitPos int) uint8 {
	byteIdx := 15 - (bitPos / 8)
	bitIdx := uint(bitPos % 8)
	return (u[byteIdx] >> bitIdx) & 1
}

func encodeShortUUID(u UUID) string {
	n := new(big.Int).SetBytes(u[:])
	if n.Sign() == 0 {
		return strings.Repeat(string(shortUUIDAlphabet[0]), shortUUIDLen)
	}

	base := big.NewInt(58)
	zero := big.NewInt(0)
	mod := new(big.Int)
	var out []byte
	for n.Cmp(zero) > 0 {
		n.DivMod(n, base, mod)
		out = append(out, shortUUIDAlphabet[mod.Int64()])
	}
	for i, j := 0, len(out)-1; i < j; i, j = i+1, j-1 {
		out[i], out[j] = out[j], out[i]
	}
	if len(out) < shortUUIDLen {
		pad := make([]byte, shortUUIDLen-len(out))
		for i := range pad {
			pad[i] = shortUUIDAlphabet[0]
		}
		out = append(pad, out...)
	}
	return string(out)
}

func decodeShortUUID(s string) (UUID, error) {
	if s == "" {
		return UUID{}, errors.New("empty shortuuid")
	}

	n := big.NewInt(0)
	base := big.NewInt(58)
	for i := 0; i < len(s); i++ {
		d := shortUUIDDecodeMap[s[i]]
		if d < 0 {
			return UUID{}, errors.New("invalid shortuuid character")
		}
		n.Mul(n, base)
		n.Add(n, big.NewInt(int64(d)))
	}
	b := n.Bytes()
	if len(b) > 16 {
		return UUID{}, errors.New("shortuuid overflows 16 bytes")
	}
	var u UUID
	copy(u[16-len(b):], b)
	return u, nil
}
