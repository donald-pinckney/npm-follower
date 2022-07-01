pub mod packument;

use serde_json::{Map, Value};

use utils::RemoveInto;

use postgres_db::change_log::Change;
use packument::Packument;


pub fn deserialize_change(c: Change) -> Option<(String, Packument)> {
    let mut change_json = serde_json::from_value::<Map<String, Value>>(c.raw_json).unwrap();
    let del = change_json.remove_key_unwrap_type::<bool>("deleted").unwrap();

    let package_name = change_json.remove_key_unwrap_type::<String>("id").unwrap();
    
    if package_name == "_design/app" || package_name == "_design/scratch" {
        return None
    }
    
    let mut doc = change_json.remove_key_unwrap_type::<Map<String, Value>>("doc").unwrap();
    let doc_id = doc.remove_key_unwrap_type::<String>("_id").unwrap();
    let doc_deleted = doc.remove_key_unwrap_type::<bool>("_deleted").unwrap_or(false);
    doc.remove_key_unwrap_type::<String>("_rev").unwrap();

    if del != doc_deleted {
        panic!("ERROR: mismatched del and del_deleted");
    }

    if package_name != doc_id {
        panic!("ERROR: mismatched package_name and doc_id");
    }

    if del {
        if doc.len() != 0 {
            panic!("ERROR: extra keys in deleted doc");
        }
        Some((package_name, Packument::Deleted))
    } else {
        let unpublished = doc
            .get("time")
            .map(|time_value| 
                time_value
                    .as_object()
                    .unwrap()
                    .contains_key("unpublished")
            )
            .unwrap_or(false);

        if unpublished {
            Some((package_name, packument::deserialize::deserialize_packument_blob_unpublished(doc)))
        } else {
            Some((package_name, packument::deserialize::deserialize_packument_blob_normal(doc)))
        }
    }    
}
