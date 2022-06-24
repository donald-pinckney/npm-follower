mod packument;

use serde_json::{Map, Value};
use postgres_db::DbConnection;
use postgres_db::internal_state;
use postgres_db::change_log;
use postgres_db::change_log::Change;
use utils::check_no_concurrent_processes;

use utils::RemoveInto;

const PAGE_SIZE: i64 = 1024;

fn main() {
    check_no_concurrent_processes("relational_db_builder");

    let conn = postgres_db::connect();

    let mut processed_up_to = internal_state::query_relational_processed_seq(&conn).unwrap_or(0);

    let num_changes_total = change_log::query_num_changes_after_seq(processed_up_to, &conn);
    let mut num_changes_so_far = 0;

    // TODO: Extract this into function (duplicated in download_queuer/src/main.rs)
    loop {
        println!("Fetching seq > {}, page size = {} ({:.1}%)", processed_up_to, PAGE_SIZE, 100.0 * (num_changes_so_far as f64) / (num_changes_total as f64));
        let changes = change_log::query_changes_after_seq(processed_up_to, PAGE_SIZE, &conn);
        let num_changes = changes.len() as i64;
        num_changes_so_far += num_changes;
        if num_changes == 0 {
            break
        }

        let last_seq_in_page = changes.last().unwrap().seq;
        
        for c in changes {
            process_change(&conn, c);
        }

        // internal_state::set_relational_processed_seq(last_seq_in_page, &conn);
        processed_up_to = last_seq_in_page;

        if num_changes < PAGE_SIZE {
            break
        }
    }
}

fn process_change(conn: &DbConnection, c: Change) {
    let mut change_json = serde_json::from_value::<Map<String, Value>>(c.raw_json).unwrap();
    let del = change_json.remove_key_unwrap_type::<bool>("deleted").unwrap();

    if del {
        panic!("Don't yet know how to handle deleted things");
    }

    let package_name = change_json.remove_key_unwrap_type::<String>("id").unwrap();

    if package_name == "_design/app" || package_name == "_design/scratch" {
        return
    }

    let doc = change_json.remove_key_unwrap_type::<Map<String, Value>>("doc").unwrap();
    let packument = packument::deserialize::deserialize_packument_blob(doc);
    println!("parsed packument: {:#?}", packument);

}
