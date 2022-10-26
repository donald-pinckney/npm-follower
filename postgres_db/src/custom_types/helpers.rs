use diesel::{deserialize, result::UnexpectedNullError};

pub fn not_none<T>(x: Option<T>) -> deserialize::Result<T> {
    x.ok_or_else(|| Box::new(UnexpectedNullError) as Box<dyn std::error::Error + Send + Sync>)
}
