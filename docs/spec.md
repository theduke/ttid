# TTID Specification

## Overview

TTID is a typed identifier format built on UUIDv8.

It provides:
- globally portable UUID compatibility,
- embedded millisecond timestamp,
- a compact numeric type discriminator,
- high-entropy randomness,
- human-friendly text rendering with a type prefix.

Binary transport remains standard UUID (`16` bytes).
Text transport is `<type-name>_<shortuuid>`.

## Data Model

A TTID payload contains `122` custom bits split as:

- `48` bits: Unix timestamp in milliseconds (`timestamp_ms`)
- `16` bits: type id (`type_id`)
- `58` bits: randomness (`randomness`)

Reasoning:
- UUID has `128` bits.
- UUID version field consumes `4` fixed bits.
- UUID variant field consumes `2` fixed bits.
- `128 - 6 = 122` payload bits remain.

## UUIDv8 Layout

TTID uses UUIDv8 and enforces:

- version nibble = `1000` (v8)
- variant bits = `10` (RFC variant)

The payload bits are packed in big-endian bit order across all non-fixed UUID bits.
This is a timestamp-first layout: timestamp occupies the most significant payload bits.

Conceptually:

1. Build payload:
   - `payload = (timestamp_ms << 74) | (type_id << 58) | randomness`
2. Write payload bits into UUID bit positions excluding version/variant bits.
3. Set fixed UUID bits for version and variant.

## Numeric Limits

- `timestamp_ms` range: `0..=2^48-1` (`0..=281_474_976_710_655`)
- `type_id` range: `0..=2^16-1` (`0..=65_535`)
- `randomness` range: `0..=2^58-1` (`0..=288_230_376_151_711_743`)

`randomness` inputs wider than 60 bits are masked to 60 bits.

## Type Domain Contract

Users define a type domain with `IdType`:

- `to_type_id(self) -> u16`
- `from_type_id(u16) -> Option<Self>`
- `as_type_name(self) -> &'static str`
- `from_type_name(&str) -> Option<Self>`

Requirements for correctness:

- mapping must be stable for persisted data,
- ids used by `to_type_id` must fit in 16 bits,
- names should be stable and URL-safe for external usage,
- `to_*` and `from_*` mappings should be bijective inside your domain.

## String Format

Canonical text format:

`<type-name>_<shortuuid>`

Where:
- `<type-name>` comes from `IdType::as_type_name`,
- `<shortuuid>` is the short-uuid base58 representation of the raw UUID.

Example:

`user_hnP4K7MiDaGjM9R6vMshtY`

## Parsing Rules

Parsing (`FromStr`) follows this order:

1. Split input on first underscore (`_`), must yield two parts.
2. Resolve `<type-name>` via `IdType::from_type_name`.
3. Parse `<shortuuid>` to UUID.
4. Validate UUID version/variant as TTID UUIDv8.
5. Decode embedded `type_id` and resolve with `IdType::from_type_id`.
6. Ensure parsed type name matches embedded type id.

Failure returns a specific `ParseTtidError` variant.

## Ordering Notes

Timestamp is embedded and can be extracted quickly.
Textual TTID strings are not guaranteed to sort by time.
Raw UUID binary ordering is timestamp-first and therefore generally gives better B-tree locality.
For strict semantic ordering, sort by extracted `timestamp_ms` and then by `type_id`/`randomness`.

## Interoperability

- Binary: standard UUID, can be stored in UUID columns.
- Text: canonical custom format, friendly for logs and APIs.
- Decoding requires the same `IdType` domain mapping.

## Security / Privacy Considerations

- Type is intentionally exposed in text form.
- Timestamp is intentionally embedded in UUID payload.
- If opaque identifiers are required, do not expose formatted TTIDs publicly.
