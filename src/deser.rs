use uuid::Uuid;

pub(super) const TIMESTAMP_BITS: u32 = 48;
pub(super) const TYPE_BITS: u32 = 16;
pub(super) const RANDOM_BITS: u32 = 58;

pub(super) const TIMESTAMP_MAX: u64 = (1u64 << TIMESTAMP_BITS) - 1;
pub(super) const TYPE_ID_MAX: u16 = u16::MAX;
pub(super) const RANDOM_MASK: u64 = (1u64 << RANDOM_BITS) - 1;

const PAYLOAD_BITS: u32 = TIMESTAMP_BITS + TYPE_BITS + RANDOM_BITS;

pub(super) fn encode_payload_to_uuid(payload: u128) -> Uuid {
    let mut bytes = [0u8; 16];

    let mut payload_bit_idx = PAYLOAD_BITS as i32 - 1;
    for uuid_bit_pos in (0..128).rev() {
        if is_fixed_uuid_bit(uuid_bit_pos) {
            continue;
        }

        let bit = ((payload >> payload_bit_idx) & 1) as u8;
        set_bit(&mut bytes, uuid_bit_pos, bit);
        payload_bit_idx -= 1;
    }

    // UUIDv8 version field (`1000`)
    set_bit(&mut bytes, 79, 1);
    set_bit(&mut bytes, 78, 0);
    set_bit(&mut bytes, 77, 0);
    set_bit(&mut bytes, 76, 0);

    // RFC variant bits (`10`)
    set_bit(&mut bytes, 63, 1);
    set_bit(&mut bytes, 62, 0);

    Uuid::from_bytes(bytes)
}

pub(super) fn decode_payload_from_uuid(uuid: Uuid) -> Option<u128> {
    let bytes = uuid.as_bytes();

    if !is_valid_ttid_uuid(bytes) {
        return None;
    }

    let mut payload = 0u128;
    for uuid_bit_pos in (0..128).rev() {
        if is_fixed_uuid_bit(uuid_bit_pos) {
            continue;
        }

        payload <<= 1;
        payload |= get_bit(bytes, uuid_bit_pos) as u128;
    }

    Some(payload)
}

fn is_valid_ttid_uuid(bytes: &[u8; 16]) -> bool {
    let version_ok = (bytes[6] >> 4) == 0b1000;
    let variant_ok = (bytes[8] & 0b1100_0000) == 0b1000_0000;
    version_ok && variant_ok
}

fn is_fixed_uuid_bit(bit_pos: usize) -> bool {
    matches!(bit_pos, 79 | 78 | 77 | 76 | 63 | 62)
}

fn set_bit(bytes: &mut [u8; 16], bit_pos: usize, value: u8) {
    let byte_idx = 15 - (bit_pos / 8);
    let bit_idx = bit_pos % 8;

    if value == 0 {
        bytes[byte_idx] &= !(1 << bit_idx);
    } else {
        bytes[byte_idx] |= 1 << bit_idx;
    }
}

fn get_bit(bytes: &[u8; 16], bit_pos: usize) -> u8 {
    let byte_idx = 15 - (bit_pos / 8);
    let bit_idx = bit_pos % 8;
    (bytes[byte_idx] >> bit_idx) & 1
}
