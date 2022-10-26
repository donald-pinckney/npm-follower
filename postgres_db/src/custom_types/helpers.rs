use diesel::{deserialize, result::UnexpectedNullError};

pub fn deserialize_not_none(bytes: Option<&[u8]>) -> deserialize::Result<&[u8]> {
    bytes.ok_or_else(|| Err(Box::new(UnexpectedNullError)))
}
