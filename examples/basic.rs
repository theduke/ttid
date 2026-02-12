use std::str::FromStr;

use ttid::{IdType, Ttid};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MyType {
    User,
    Org,
}

impl IdType for MyType {
    fn to_type_id(self) -> u16 {
        match self {
            Self::User => 1,
            Self::Org => 2,
        }
    }

    fn from_type_id(id: u16) -> Option<Self> {
        match id {
            1 => Some(Self::User),
            2 => Some(Self::Org),
            _ => None,
        }
    }

    fn as_type_name(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Org => "org",
        }
    }

    fn from_type_name(name: &str) -> Option<Self> {
        match name {
            "user" => Some(Self::User),
            "org" => Some(Self::Org),
            _ => None,
        }
    }
}

fn main() {
    let user_id = Ttid::<MyType>::new(MyType::User).expect("id generation must succeed");

    println!("ttid: {user_id}");
    println!("uuid: {}", user_id.as_uuid());
    println!("timestamp_ms: {}", user_id.timestamp_ms());

    let text = user_id.to_string();
    let parsed = Ttid::<MyType>::from_str(&text).expect("roundtrip parse must succeed");

    assert_eq!(parsed, user_id);
    assert_eq!(parsed.id_type(), MyType::User);
}
