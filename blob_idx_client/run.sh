#!/bin/bash
# This script runs the blob_idx_client rust binary
# it builds it if it doesn't exist


# get path of this script
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"

# go one directory up
cd $DIR/..

# build if it doesn't exist
if [ ! -f target/release/blob_idx_client ]; then
    cargo build --release --quiet 2>/dev/null
fi

# run (use all the arguments passed to this script)
target/release/blob_idx_client $@
