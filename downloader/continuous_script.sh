#!/bin/bash

# runs download_queuer and downloader in a loop


# get from argv1 if "cluster" or "local"
if [ "$1" == "cluster" ]; then
    echo "Running on cluster"
    RUST_BIN="cluster_downloader"
elif [ "$1" == "local" ]; then
    echo "Running locally"
    RUST_BIN="downloader"
else
  echo "Please specify 'cluster' or 'local' as first argument"
  exit 1
fi

# get from argv2 the amount of workers
if [ "$2" -gt 0 ]; then
    echo "Running with $2 workers"
    WORKERS=$2
else
  echo "Please specify the amount of workers as second argument"
  exit 1
fi

# get path to this script
SCRIPT_PATH=$(dirname $(readlink -f $0))
# cd to parent of this script
cd $SCRIPT_PATH/..


# loop forever
while true; do 
  # exit if download_queuer or downloader exit with error
  cargo run --release --bin download_queuer || exit 1
  cargo run --release --bin $RUST_BIN -- $WORKERS || exit 1
done

