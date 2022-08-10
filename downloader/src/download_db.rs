use postgres_db::{
    download_queue::{
        get_total_tasks_num, load_chunk_init, load_chunk_next, update_from_error,
        update_from_tarballs, DownloadTask, TASKS_CHUNK_SIZE,
    },
    download_tarball::DownloadedTarball,
    DbConnection,
};
use std::{os::unix::prelude::PermissionsExt, sync::mpsc::channel};

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
        .timeout(std::time::Duration::from_secs(600)) // timeout of 10 minutes
        .build()?;

    let res = client.get(&task.url).send().await?;
    let status = res.status();
    if status != reqwest::StatusCode::OK {
        return Err(DownloadError::StatusNotOk(status));
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
pub fn update_from_tarball_queue(conn: &DbConnection, tarballs: &mut Vec<DownloadedTarball>) {
    if tarballs.is_empty() {
        return;
    }
    update_from_tarballs(conn, &tarballs);
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
    conn: &DbConnection,
    dest: &str,
    num_workers: usize,
    retry_failed: bool,
) -> std::io::Result<()> {
    assert!(TASKS_CHUNK_SIZE > num_workers as i64 && num_workers > 0);

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
    // the last chunk size that was quried
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
