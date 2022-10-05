#!/bin/bash

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
seq=$1

file=$SCRIPT_DIR/resources/bench_many_changes/random_sample_100.jsonl
if [ -s "$file" ]
then
    echo "Skipping generating $file"
else
    echo "Generating $file by selecting a random sample of 100 changes (ETA: ~5 seconds)"
    time psql -p 5431 -h 127.0.0.1 npm_data -c "WITH random_sample AS (SELECT * FROM change_log ORDER BY RANDOM() LIMIT(100)) SELECT raw_json FROM random_sample ORDER BY seq;" -t -A -o $file
fi



file=$SCRIPT_DIR/resources/bench_many_changes/random_sample_1000.jsonl
if [ -s "$file" ]
then
    echo "Skipping generating $file"
else
    echo "Generating $file by selecting a random sample of 1000 changes (ETA: ~45 seconds)"
    time psql -p 5431 -h 127.0.0.1 npm_data -c "WITH random_sample AS (SELECT * FROM change_log ORDER BY RANDOM() LIMIT(1000)) SELECT raw_json FROM random_sample ORDER BY seq;" -t -A -o $file
fi


file=$SCRIPT_DIR/resources/bench_many_changes/random_sample_10000.jsonl
if [ -s "$file" ]
then
    echo "Skipping generating $file"
else
    echo "Generating $file by selecting a random sample of 10000 changes (ETA: ~3 minutes)"
    time psql -p 5431 -h 127.0.0.1 npm_data -c "WITH random_sample AS (SELECT * FROM change_log ORDER BY RANDOM() LIMIT(10000)) SELECT raw_json FROM random_sample ORDER BY seq;" -t -A -o $file
fi


file=$SCRIPT_DIR/resources/bench_many_changes/first_1000.jsonl
if [ -s "$file" ]
then
    echo "Skipping generating $file"
else
    echo "Generating $file by selecting the first 1000 changes (ETA: ~3 seconds)"
    time psql -p 5431 -h 127.0.0.1 npm_data -c "SELECT raw_json FROM change_log ORDER BY seq LIMIT(1000);" -t -A -o $file
fi

file=$SCRIPT_DIR/resources/bench_many_changes/first_10000.jsonl
if [ -s "$file" ]
then
    echo "Skipping generating $file"
else
    echo "Generating $file by selecting the first 10000 changes (ETA: ~5 seconds)"
    time psql -p 5431 -h 127.0.0.1 npm_data -c "SELECT raw_json FROM change_log ORDER BY seq LIMIT(10000);" -t -A -o $file
fi

file=$SCRIPT_DIR/resources/bench_many_changes/first_100000.jsonl
if [ -s "$file" ]
then
    echo "Skipping generating $file"
else
    echo "Generating $file by selecting the first 100000 changes (ETA: ~6 seconds)"
    time psql -p 5431 -h 127.0.0.1 npm_data -c "SELECT raw_json FROM change_log ORDER BY seq LIMIT(100000);" -t -A -o $file
fi

file=$SCRIPT_DIR/resources/bench_many_changes/first_200000.jsonl
if [ -s "$file" ]
then
    echo "Skipping generating $file"
else
    echo "Generating $file by selecting the first 200000 changes (ETA: ~20 seconds)"
    time psql -p 5431 -h 127.0.0.1 npm_data -c "SELECT raw_json FROM change_log ORDER BY seq LIMIT(200000);" -t -A -o $file
fi

file=$SCRIPT_DIR/resources/bench_many_changes/last_1000.jsonl
if [ -s "$file" ]
then
    echo "Skipping generating $file"
else
    echo "Generating $file by selecting the last 1000 changes (ETA: ~15 seconds)"
    time psql -p 5431 -h 127.0.0.1 npm_data -c "SELECT raw_json FROM (SELECT * FROM change_log ORDER BY seq DESC LIMIT 1000) as stuff ORDER BY seq ASC;" -t -A -o $file
fi

file=$SCRIPT_DIR/resources/bench_many_changes/last_10000.jsonl
if [ -s "$file" ]
then
    echo "Skipping generating $file"
else
    echo "Generating $file by selecting the last 10000 changes (ETA: ~5 minutes)"
    time psql -p 5431 -h 127.0.0.1 npm_data -c "SELECT raw_json FROM (SELECT * FROM change_log ORDER BY seq DESC LIMIT 10000) as stuff ORDER BY seq ASC;" -t -A -o $file
fi