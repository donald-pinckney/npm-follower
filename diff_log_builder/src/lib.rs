// pub mod deserialize;

use std::collections::HashMap;
use std::collections::HashSet;
use std::panic;
use postgres_db::diff_log;
use postgres_db::diff_log::NewDiffLogEntry;
use postgres_db::diff_log::NewDiffLogEntryWithHash;
use postgres_db::diff_log::internal_diff_log_state::manager::DiffStateManager;
use postgres_db::packages::Package;
use postgres_db::versions::Version;
use postgres_db::DbConnection;
use postgres_db::change_log::Change;
use postgres_db::custom_types::Semver;
use postgres_db::dependencies::Dependencie;

use serde_json::{Map, Value};

use utils::RemoveInto;


pub fn process_changes(conn: &DbConnection, changes: Vec<Change>) {
    let mut state_manager = DiffStateManager::new();
    let mut new_diff_entries: Vec<NewDiffLogEntryWithHash> = Vec::new();
    
    for c in changes {
        process_change(conn, c, &mut state_manager, &mut new_diff_entries);
    }
    state_manager.flush_to_db(conn);
    diff_log::insert_diff_log_entries(new_diff_entries.into_iter().map(|x| x.entry).collect(), conn);
}

fn process_change(conn: &DbConnection, c: Change, state_manager: &mut DiffStateManager, new_diff_entries: &mut Vec<NewDiffLogEntryWithHash>) {
    let seq = c.seq;

    // Parse the Change
    let result = panic::catch_unwind(|| deserialize_change(c));
    let (package_name, packument) = match result {
        Err(err) => {
            println!("Failed on seq: {}", seq);
            panic::resume_unwind(err);
        }
        Ok(Some((name, pack))) => (name, pack),
        Ok(None) => return,
    };
    
    // 1. Lookup current state
    // 2. Decide what type of change it is (look at hashes, etc.) and compute diffs
    let diff_instrs: Vec<NewDiffLogEntryWithHash> = match state_manager.lookup_package(package_name.clone(), conn) {
        Some(hash_state) => {
            vec![]
        },
        None => {
            // 1. 
            vec![]
        }
    };

    // 3. Update state
    for d in &diff_instrs {
        state_manager.apply_diff_entry(&d);
    }
    // 4. Add diff entries
    new_diff_entries.extend(diff_instrs);
}



pub fn deserialize_change(c: Change) -> Option<(String, ())> {
    todo!()

    // let mut change_json = serde_json::from_value::<Map<String, Value>>(c.raw_json).unwrap();
    // let del = change_json.remove_key_unwrap_type::<bool>("deleted").unwrap();

    // let package_name = change_json.remove_key_unwrap_type::<String>("id").unwrap();
    
    // if package_name == "_design/app" || package_name == "_design/scratch" {
    //     return None
    // }
    
    // let mut doc = change_json.remove_key_unwrap_type::<Map<String, Value>>("doc").unwrap();
    // let doc_id = doc.remove_key_unwrap_type::<String>("_id").unwrap();
    // let doc_deleted = doc.remove_key_unwrap_type::<bool>("_deleted").unwrap_or(false);
    // doc.remove_key_unwrap_type::<String>("_rev").unwrap();

    // if del != doc_deleted {
    //     panic!("ERROR: mismatched del and del_deleted");
    // }

    // if package_name != doc_id {
    //     panic!("ERROR: mismatched package_name and doc_id");
    // }

    // if del {
    //     if !doc.is_empty() {
    //         panic!("ERROR: extra keys in deleted doc");
    //     }
    //     Some((package_name, Packument::Deleted))
    // } else {
    //     let unpublished = doc
    //         .get("time")
    //         .map(|time_value| 
    //             time_value
    //                 .as_object()
    //                 .unwrap()
    //                 .contains_key("unpublished")
    //         )
    //         .unwrap_or(false);

    //     if unpublished {
    //         Some((package_name, packument::deserialize::deserialize_packument_blob_unpublished(doc)))
    //     } else {
    //         let has_dist_tags = doc.contains_key("dist-tags");
    //         if has_dist_tags {
    //             Some((package_name, packument::deserialize::deserialize_packument_blob_normal(doc)))
    //         } else {
    //             // If the packument says *not* deleted, 
    //             // but has no fields, then we mark it as missing data.
    //             // See seq = 4413127.
    //             assert!(!doc.contains_key("time"));
    //             assert!(!doc.contains_key("versions"));
    //             Some((package_name, Packument::MissingData))
    //         }
    //     }
    // }    
}




