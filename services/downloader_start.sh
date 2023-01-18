#!/bin/bash
cd /zfs-raidz1/XXXXXX/npm-follower
cargo run --release --bin download_queuer
# might get stuck due to disk issues, so lets put a 10hr timeout
timeout 10h cargo run --release --bin downloader ../tarballs/ 30
