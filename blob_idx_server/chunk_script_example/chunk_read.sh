#!/bin/bash

BLOB_FILE=./chunk
OFFSET_FILE=$1

# check if argument is given
if [ -z "$1" ]; then
    echo "Usage: $0 <offset_file>"
    exit 1
fi

# check if offset file exists
if [ ! -f "$OFFSET_FILE" ]; then
    echo "Offset file does not exist"
    exit 1
fi

# parse "offset,size" from offset file
OFFSET=$(cat $OFFSET_FILE | cut -d, -f1)
SIZE=$(cat $OFFSET_FILE | cut -d, -f2)

# read chunk from blob file and print to stdout
dd if=$BLOB_FILE bs=1 skip=$OFFSET count=$SIZE 2>/dev/null
