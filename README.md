# ttid

Typed, time-aware IDs built on UUIDv8.

`ttid` is for systems where you want UUID interoperability, but better ergonomics in logs and APIs.
It keeps IDs as real UUIDs (`16` bytes, standard storage/index support) while adding two practical improvements:

- IDs carry a type discriminator.
- IDs render as readable strings: `<type-name>_<shortuuid>`.

Example:

`user_hnP4K7MiDaGjM9R6vMshtY`

## Why this exists

Plain UUIDs are great for uniqueness and distribution, but they are weak for day-to-day operations:

- a random UUID in logs gives little context,
- mixed entity IDs are easy to confuse,
- debugging often requires extra lookups just to learn what an ID refers to.

`ttid` addresses that without giving up UUID compatibility.

## What a TTID contains

Each TTID packs these fields into UUIDv8 payload bits:

- `48-bit` millisecond Unix timestamp
- `16-bit` type id
- `58-bit` randomness

Important detail: packing is **timestamp-first** in the UUID payload, which improves locality for UUID-ordered indexes in common databases.

## String format

Canonical text format:

`<type-name>_<shortuuid>`

- `<type-name>` comes from your `IdType` mapping.
- `<shortuuid>` is generated via the `short-uuid` crate.

`short-uuid` uses the Flickr Base58 alphabet, which is easier to work with than dashed hex UUID text:

- shorter and cleaner in logs and terminals,
- no punctuation noise from dash-separated groups,
- avoids commonly confused characters during copy/typing.

Parsing validates both parts:

- the type name must be known,
- the UUID must be a valid TTID layout,
- the embedded numeric type id must match the parsed type name.

## Quick usage

```rust
use std::str::FromStr;
use ttid::{IdType, Ttid};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MyType {
    User,
    Session,
}

impl IdType for MyType {
    fn to_type_id(self) -> u16 {
        match self {
            Self::User => 1,
            Self::Session => 2,
        }
    }

    fn from_type_id(id: u16) -> Option<Self> {
        match id {
            1 => Some(Self::User),
            2 => Some(Self::Session),
            _ => None,
        }
    }

    fn as_type_name(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Session => "session",
        }
    }

    fn from_type_name(name: &str) -> Option<Self> {
        match name {
            "user" => Some(Self::User),
            "session" => Some(Self::Session),
            _ => None,
        }
    }
}

let id = Ttid::<MyType>::new(MyType::User).unwrap();

// Human-friendly text for logs/APIs
let text = id.to_string();

// Parse and validate back
let parsed = Ttid::<MyType>::from_str(&text).unwrap();
assert_eq!(parsed, id);
assert_eq!(parsed.id_type(), MyType::User);

// Access raw UUID when needed
let raw_uuid = parsed.as_uuid();
```

## When this is a good fit

Use `ttid` when you want:

- UUID-native storage and tooling,
- readable IDs in logs, metrics, and APIs,
- explicit typed ID domains in Rust,
- coarse creation-time information embedded in the ID.

## Tradeoffs

Be explicit about these:

- Timestamp is embedded, so IDs are not fully opaque.
- Type information is intentionally visible in text form.
- Textual TTIDs are optimized for readability, not lexical time ordering.

## Documentation

- Full format and bit-level details: [`docs/spec.md`](docs/spec.md)
- Runnable usage example: [`examples/basic.rs`](examples/basic.rs)

## Develop

### Release Automation

GitHub Actions uses `release-plz` via `.github/workflows/release-plz.yml`.

- On pushes to `main`, it opens or updates a release PR with version/changelog changes.
- If `CARGO_REGISTRY_TOKEN` is configured in repository secrets, it also runs publish/release.
