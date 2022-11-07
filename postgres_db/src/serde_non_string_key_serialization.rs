use serde::{Deserialize, Serialize};
use serde::{Deserializer, Serializer};

pub fn serialize<'a, M, K, V, S>(map: M, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    M: IntoIterator<Item = (&'a K, &'a V)>,
    K: 'a,
    V: 'a,
    K: Serialize,
    V: Serialize,
{
    let map_items: Vec<_> = map.into_iter().collect();
    map_items.serialize(serializer)
}

pub fn deserialize<'de, M, K, V, D>(deserializer: D) -> Result<M, D::Error>
where
    D: Deserializer<'de>,
    M: FromIterator<(K, V)>,
    K: Deserialize<'de>,
    V: Deserialize<'de>,
{
    let map_items = <Vec<(K, V)> as Deserialize>::deserialize(deserializer)?;
    Ok(M::from_iter(map_items))
}
