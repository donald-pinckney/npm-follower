#!/bin/bash
cd /zfs-raidz1/federico/npm-follower
cargo run --release --bin download_queuer
cargo run --release --bin downloader ../tarballs/ 30
