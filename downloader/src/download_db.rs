use blob_idx_server::errors::{BlobError, ClientError};
use blob_idx_server::http::{JobType, SubmitJobRequest};
use postgres_db::connection::DbConnection;
use postgres_db::custom_types::DownloadFailed;
use postgres_db::download_queue::{
    get_total_tasks_num, load_chunk_init, load_chunk_next, update_from_error, update_from_tarballs,
    DownloadTask, TASKS_CHUNK_SIZE,
};
use postgres_db::download_tarball::DownloadedTarball;
use std::collections::HashMap;
use std::{os::unix::prelude::PermissionsExt, sync::mpsc::channel};
use tokio::task::JoinHandle;

use crate::{
    download_error::DownloadError,
    download_threadpool::{DbMessage, DownloadThreadPool},
};

/// Downloads the given task to the given directory. This function cannot panic.
pub async fn download_task(
    task: &DownloadTask,
    dest: &str,
) -> Result<DownloadedTarball, DownloadError> {
    // get the file and download it to dir
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300)) // timeout of 5 minutes
        .build()?;

    let res = client.get(&task.url).send().await?;
    let status = res.status();
    if status != reqwest::StatusCode::OK {
        return Err(DownloadError::StatusNotOk(status.as_u16()));
    }

    let name = DownloadTask::get_filename(&task.url)?;
    let path = std::path::Path::new(dest).join(name);
    let mut file = std::fs::File::create(path.clone())?;
    file.set_permissions(std::fs::Permissions::from_mode(0o774))?; // rwxrwxr--
    let mut body = std::io::Cursor::new(res.bytes().await?);
    std::io::copy(&mut body, &mut file)?;

    let downloaded_tarball = DownloadedTarball::from_task(
        task,
        // makes the path absolute
        std::fs::canonicalize(path)?
            .to_str()
            .ok_or(DownloadError::BadlyFormattedUrl)?
            .to_string(),
    );

    Ok(downloaded_tarball)
}

/// Updates the database with the given tarballs and then clears the queue.
pub fn update_from_tarball_queue(conn: &mut DbConnection, tarballs: &mut Vec<DownloadedTarball>) {
    if tarballs.is_empty() {
        return;
    }
    update_from_tarballs(conn, tarballs);
    tarballs.clear();
}

/// Downloads all present tasks to the given directory. Inserts each task completed in the
/// downloaded_tarballs table, and removes the completed tasks from the download_tasks table.
/// The given number of workers represents the number of threads that will be used to download the
/// tasks, where for each thread there is a new parallel download. if retry_failed is true, it will
/// retry to download failed tasks.
///
/// # Panics
///
/// If the number of workers is 0 or greater than TASKS_CHUNK_SIZE (unreasonable amount).
pub fn download_to_dest(
    conn: &mut DbConnection,
    dest: &str,
    num_workers: usize,
    retry_failed: bool,
) -> std::io::Result<()> {
    assert!(TASKS_CHUNK_SIZE > num_workers && num_workers > 0);

    // get all tasks with no failed downloads
    let tasks_len = get_total_tasks_num(conn, retry_failed);
    println!("{} tasks to download", tasks_len);

    let (db_sender, db_receiver) = channel();
    let pool = DownloadThreadPool::new(num_workers, dest, db_sender);

    // get first round of tasks, with no failed downloads
    let tasks: Vec<DownloadTask> = load_chunk_init(conn, retry_failed);
    println!("Got {} tasks", tasks.len());

    if tasks.is_empty() {
        return Ok(());
    }

    // ---  variables to keep for safely querying new chunks of tasks ---

    // the last url of the task that was queried
    let mut last_url = tasks.last().unwrap().url.clone();
    // the last chunk size that was queried
    let mut last_chunk_size = tasks.len();
    // the counter of downloads per chunk (gets reset on each chunk)
    let mut download_counter = 0;
    // the queue of tarballs that have been downloaded and are waiting to be inserted into the db
    let mut tarballs_queue: Vec<DownloadedTarball> = vec![];

    // pool the first round of tasks
    pool.download_chunk(tasks);

    for i in 0..tasks_len {
        println!(
            "Status: {}/{} - Chunkwise: {}/{}",
            i, tasks_len, download_counter, last_chunk_size
        );
        if (download_counter + 1) == last_chunk_size {
            update_from_tarball_queue(conn, &mut tarballs_queue);

            println!("Sending new chunk of tasks to pool");
            // get next round of tasks, with no failed downloads and with tasks that have greater
            // url sort-position than the last chunk
            let tasks: Vec<DownloadTask> = load_chunk_next(conn, &last_url, retry_failed);
            println!("Got {} tasks", tasks.len());

            // reassign last_url and last_chunk_size to the new chunk of tasks
            if !tasks.is_empty() {
                last_url = tasks.last().unwrap().url.clone();
            }
            last_chunk_size = tasks.len();
            // reset download_counter
            download_counter = 0;

            // pool the new chunk of tasks
            pool.download_chunk(tasks);
        }
        match db_receiver.recv().unwrap() {
            // NOTE: the tarballs get inserted in chunks, but errors don't. This is because it
            // doesn't make sense to pool errors as we have to update each row in the tasks in a
            // loop.
            DbMessage::Tarball(tarball) => {
                println!("Done downloading task {}", tarball.tarball_url);
                tarballs_queue.push(*tarball);
            }
            DbMessage::Error(e, task) => {
                println!("Error downloading task {} -> {}", task.url, e);
                update_from_error(conn, &task, e.into());
            }
        }
        download_counter += 1;
    }

    update_from_tarball_queue(conn, &mut tarballs_queue);

    println!("Done downloading tasks");

    Ok(())
}

/// Downloads all present tasks to the computing cluster. Inserts each task completed in the
/// downloaded_tarballs table, and removes the completed tasks from the download_tasks table.
/// The given number of parallel dls represent the number of tarballs per worker that will be
/// downloaded in parallel. The retry_failed flag indicates whether to retry failed downloads.
pub async fn download_to_cluster(
    conn: &mut DbConnection,
    num_parallel_dl: usize,
    retry_failed: bool,
) -> std::io::Result<()> {
    let blob_api_url = std::env::var("BLOB_API_URL").expect("BLOB_API_URL not set");
    let blob_api_key = std::env::var("BLOB_API_KEY").expect("BLOB_API_KEY not set");
    let client = reqwest::Client::new();

    let req_chunk_size = ((TASKS_CHUNK_SIZE as usize) / num_parallel_dl) + 1; // make chunk smaller due to cluster overhead

    // get all tasks with no failed downloads if retry_failed is false
    let tasks_len = get_total_tasks_num(conn, retry_failed);
    println!("[MAIN] {} tasks to download", tasks_len);

    let mut tasks: Vec<DownloadTask> = load_chunk_init(conn, retry_failed);
    println!("[MAIN] Got {} tasks", tasks.len());
    while !tasks.is_empty() {
        let mut handles = vec![];

        for (worker_id, chunk) in tasks.chunks(req_chunk_size as usize).enumerate() {
            let blob_api_url = blob_api_url.clone();
            let blob_api_key = blob_api_key.clone();
            let client = client.clone();

            let mut url_to_task = HashMap::new();

            let mut urls = vec![];
            for task in chunk {
                urls.push(task.url.to_string());
                url_to_task.insert(task.url.clone(), task.clone());
            }
            // essentially, we could have two types of errors:
            // 1. Per-tarball errors (e.g. 404, 500, etc)
            // 2. Per-chunk, cluster-related errors, which make a whole chunk invalid
            type ClusterResult = Result<
                Vec<Result<DownloadedTarball, (DownloadTask, DownloadError)>>,
                (DownloadError, Vec<DownloadTask>),
            >;

            let handle: JoinHandle<ClusterResult> = tokio::spawn(async move {
                let data = SubmitJobRequest {
                    job_type: JobType::DownloadURLs { urls: urls.clone() },
                };
                let mut tbs = vec![];
                let thunk = async {
                    // loop so we can retry in case of blob-related errors
                    loop {
                        println!("[{}] Sending job to cluster", worker_id);
                        let res = client
                            .post(&format!("{}/job/submit", blob_api_url))
                            .header("Authorization", blob_api_key.clone())
                            .json(&data)
                            .send()
                            .await?;
                        let txt = res.text().await?;
                        if txt.is_empty() {
                            // success
                            println!("[{}] Downloaded {} tarballs", worker_id, urls.len());

                            for url in urls.iter() {
                                let task = url_to_task.get(url.as_str()).unwrap();
                                let downloaded =
                                    DownloadedTarball::from_task_blob(task, url.to_string());
                                tbs.push(Ok(downloaded));
                            }
                            return Ok(tbs);
                        } else {
                            // unravel the error (yes, it's a bad API)
                            println!("[{}] Error downloading tarballs: {}", worker_id, txt);
                            let obj: serde_json::Value = serde_json::from_str(&txt)
                                .map_err(|_| DownloadError::ClusterError)?;
                            let err: ClientError = serde_json::from_value(obj["error"].clone())
                                .map_err(|_| DownloadError::ClusterError)?;

                            match err {
                                // this kind of error is benign, we need to make the assumption
                                // that the tarballs were downloaded correctly here. yet,
                                // we need to resubmit the job with the missing tarballs.
                                ClientError::BlobError(BlobError::AlreadyExists(url)) => {
                                    println!("[{}] Already downloaded {}", worker_id, url);
                                    tbs.push(Ok(DownloadedTarball::from_task_blob(
                                        url_to_task.get(&url).unwrap(),
                                        url.to_string(),
                                    )));

                                    if urls.len() == 1 {
                                        // here we actually return.
                                        return Ok(tbs);
                                    } else {
                                        urls.retain(|u| u != &url);
                                    }
                                }
                                ClientError::DownloadFailed { urls: failed_urls } => {
                                    println!(
                                        "[{}] {}/{} tarball downloads failed",
                                        worker_id,
                                        failed_urls.len(),
                                        urls.len()
                                    );
                                    for (url, status) in failed_urls.iter() {
                                        let task = url_to_task.get(url.as_str()).unwrap();
                                        tbs.push(Err((
                                            task.clone(),
                                            DownloadError::StatusNotOk((*status).into()),
                                        )));
                                        urls.retain(|u| u != url);
                                    }

                                    for url in urls.iter() {
                                        let task = url_to_task.get(url.as_str()).unwrap();
                                        let downloaded = DownloadedTarball::from_task_blob(
                                            task,
                                            url.to_string(),
                                        );
                                        tbs.push(Ok(downloaded));
                                    }
                                    return Ok(tbs);
                                }
                                _ => return Err(DownloadError::ClusterError),
                            }
                        }
                    }
                };
                let res = thunk.await;
                res.map_err(|e| {
                    (
                        e,
                        urls.iter()
                            .flat_map(|u| url_to_task.get(u))
                            .cloned()
                            .collect(),
                    )
                })
            });
            handles.push(handle);
        }

        for handle in handles {
            let mut good_tbs = vec![];
            match handle.await.unwrap() {
                // here we have a list of signular download results, which can be either
                // Ok(DownloadedTarball) or Err((DownloadTask, DownloadError))
                Ok(tbs) => {
                    for tb in tbs {
                        match tb {
                            Ok(tb) => good_tbs.push(tb),
                            Err((task, err)) => {
                                update_from_error(conn, &task, err.into());
                            }
                        }
                    }
                }
                // means cluster error, not per-tarball error
                Err((e, tasks)) => {
                    let sql_err: DownloadFailed = e.into();
                    println!("[MAIN] Updating {} failed tasks", tasks.len());
                    for task in tasks {
                        update_from_error(conn, &task, sql_err.clone());
                    }
                }
            }

            if !good_tbs.is_empty() {
                // NOTE: there is a print statement in update_from_tarballs
                update_from_tarballs(conn, &good_tbs);
            }
        }

        // refill tasks
        tasks = load_chunk_next(conn, &tasks.last().unwrap().url, retry_failed);
    }

    println!("Done downloading tasks");

    Ok(())
}
