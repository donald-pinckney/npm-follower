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
  # exit if download_queuer or downloader exit with error.
  cargo run --release --bin download_queuer || exit 1

  # if downloader returns "0 tasks to download" then sleep for 30 seconds
  exec 5>&1
  set -o pipefail
  OUTPUT=$(cargo run --release --bin $RUST_BIN -- $WORKERS 2>&1 | tee >(cat - >&5))
  # check $? for exit code
  if [ $? -eq 1 ]; then
    exit 1
  fi
  set +o pipefail
  # check if first 1000 lines of output contains "0 tasks to download"
  if echo "$OUTPUT" | head -n 1000 | grep -q "0 tasks to download"; then
    echo "Sleeping for 30 seconds"
    sleep 30
  fi
done

