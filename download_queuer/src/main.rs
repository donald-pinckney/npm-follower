use utils::check_no_concurrent_processes;
use postgres_db::change_log::Change;
use postgres_db::internal_state;
use postgres_db::download_queue;
use postgres_db::change_log;

const PAGE_SIZE: i64 = 1024;

fn main() {
    check_no_concurrent_processes("download_queuer");


    let conn = postgres_db::connect();

    let mut queued_up_to = internal_state::query_queued_downloads_seq(&conn).unwrap_or(0);

    let num_changes_total = change_log::query_num_changes_after_seq(queued_up_to, &conn);
    let mut num_changes_so_far = 0;

    loop {
        println!("Fetching seq > {}, page size = {} ({:.1}%)", queued_up_to, PAGE_SIZE, 100.0 * (num_changes_so_far as f64) / (num_changes_total as f64));
        let changes = change_log::query_changes_after_seq(queued_up_to, PAGE_SIZE, &conn);
        let num_changes = changes.len() as i64;
        num_changes_so_far += num_changes;
        if num_changes == 0 {
            break
        }

        let last_seq_in_page = changes.last().unwrap().seq;
        
        let downloads_to_enqueue: Vec<_> = changes.into_iter()
            .flat_map(|c| download_tasks_for_change(c))
            .collect();

        download_queue::enqueue_downloads(downloads_to_enqueue, &conn);
        internal_state::set_queued_downloads_seq(last_seq_in_page, &conn);
        queued_up_to = last_seq_in_page;

        if num_changes < PAGE_SIZE {
            break
        }
    }
    
}


trait BetterUnwrap {
    type Wrapped;
    fn unwrap_debug<F>(self, f: F) -> Self::Wrapped where F: FnOnce() -> String;
}

impl<T> BetterUnwrap for Option<T> {
    type Wrapped = T;
    fn unwrap_debug<F>(self, f: F) -> T where F: FnOnce() -> String {
        match self {
            Some(x) => x,
            None => { panic!("Expected Some, received None. Reason: {}", f()) }
        }
    }
}

impl<T, R> BetterUnwrap for Result<T, R> where R: std::fmt::Debug {
    type Wrapped = T;
    fn unwrap_debug<F>(self, f: F) -> T where F: FnOnce() -> String {
        match self {
            Ok(x) => x,
            Err(e) => { panic!("Expected Ok, received Err({:?}). Reason: {}", e, f()) }
        }
    }
}



pub fn download_tasks_for_change(change: Change) -> Vec<download_queue::DownloadTask> {
    let seq = change.seq;
    let seq_debug_print = |note| { move || { format!("{} (seq = {})", note, seq) } };

    let j = change.raw_json;

    let deleted = j
        .get("deleted")
        .unwrap_debug(seq_debug_print("Expected deleted field"))
        .as_bool()
        .unwrap_debug(seq_debug_print("Deleted field must be a boolean"));
    
    if deleted {
        return vec![]
    }

    let package = j
        .get("id")
        .unwrap_debug(seq_debug_print("Expected id field"))
        .as_str()
        .unwrap_debug(seq_debug_print("Id field must be a string"));

    if package == "_design/app" || package == "_design/scratch" {
        return vec![]
    }

    let doc = j
        .get("doc")
        .unwrap_debug(seq_debug_print("Expected doc field"))
        .as_object()
        .unwrap_debug(seq_debug_print("Doc field must be an object"));

    let doc_id = doc
        .get("_id")
        .unwrap_debug(seq_debug_print("Expected _id field in doc"))
        .as_str()
        .unwrap_debug(seq_debug_print("_id field must be a string"));

    if doc_id != package {
        panic!("Doc _id does not match id (seq = {})", seq);
    }

    let empty_map = serde_json::map::Map::new();
    let versions = match doc.get("versions") {
        Some(versions) => versions
            .as_object()
            .unwrap_debug(seq_debug_print("Versions field must be an object")),

        // Note: A package doc might have no versions field in the case that the versions have been unpublished. See seq = 1783991
        None => &empty_map
    };

    versions.iter().map(|(_v_version, v_data)| {
        let dist = v_data
            .get("dist")
            .unwrap_debug(seq_debug_print("Expected dist field"))
            .as_object()
            .unwrap_debug(seq_debug_print("dist field must be an object"));

        let sig0 = dist
            .get("signatures")
            .and_then(|sigs| 
                sigs
                    .as_array()
                    .unwrap_debug(seq_debug_print("signatures must be an array"))
                    .first()
                    .map(|s| 
                        s.as_object().unwrap_debug(seq_debug_print("signature must be an object"))));

        download_queue::DownloadTask::fresh_task(
            dist.get("tarball")
                .unwrap_debug(seq_debug_print("Missing tarball field"))
                .as_str()
                .unwrap_debug(seq_debug_print("Tarball field must be a string"))
                .to_owned(),
            dist.get("shasum")
                .map(|s| s.as_str()
                            .unwrap_debug(seq_debug_print("shasum must be a string"))
                            .to_owned()),
            dist.get("unpackedSize")
                .map(|s| s.as_i64()
                            .unwrap_debug(seq_debug_print("unpackedSize must be a number"))),
            dist.get("fileCount")
                .map(|s| s.as_i64()
                            .unwrap_debug(seq_debug_print("fileCount must be a number"))
                            .try_into()
                            .unwrap_debug(seq_debug_print("too many files to fit in i32"))),
            dist.get("integrity")
                .map(|s| s.as_str()
                            .unwrap_debug(seq_debug_print("integrity must be a string"))
                            .to_owned()),
            sig0.map(|s| s.get("sig")
                            .unwrap_debug(seq_debug_print("Missing sig field"))
                            .as_str()
                            .unwrap_debug(seq_debug_print("sig must be a string"))
                            .to_owned()),
            sig0.map(|s| s.get("keyid")
                            .unwrap_debug(seq_debug_print("Missing keyid field"))
                            .as_str()
                            .unwrap_debug(seq_debug_print("keyid must be a string"))
                            .to_owned()),
            dist.get("npm-signature")
                .map(|s| s.as_str()
                            .unwrap_debug(seq_debug_print("npm-signature must be a string"))
                            .to_owned())
        )
    }).collect()
}