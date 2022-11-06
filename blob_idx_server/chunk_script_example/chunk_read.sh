#!/bin/bash

BLOB_FILE=$1
OFFSET_FILE=$2

# check if arguments are provided
if [ -z "$BLOB_FILE" ] || [ -z "$OFFSET_FILE" ]; then
    echo "Usage: $0 <blob_file> <offset_file>"
    exit 1
fi


# check if offset file exists
if [ ! -f "$OFFSET_FILE" ]; then
    echo "Offset file does not exist"
    exit 1
fi

# check if blob file exists
if [ ! -f "$BLOB_FILE" ]; then
    echo "Blob file does not exist"
    exit 1
fi

# parse "offset,size" from offset file
OFFSET=$(cat $OFFSET_FILE | cut -d, -f1)
SIZE=$(cat $OFFSET_FILE | cut -d, -f2)

# read chunk from blob file and print to stdout
dd if=$BLOB_FILE bs=1 skip=$OFFSET count=$SIZE 2>/dev/null
