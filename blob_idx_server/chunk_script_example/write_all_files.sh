#!/bin/bash


# for all files in ./files, use ./chunk_write.sh to write them to the blob

for file in ./files/* ; do
    ./chunk_write.sh $file ./offsets/$(basename $file)
done
