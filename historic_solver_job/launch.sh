#!/bin/bash

npm_cache_dir=$(mktemp -d 2>/dev/null || mktemp -d -t 'mytmpdir')


if [[ -z "$SLURM_JOB_ID" ]];
then
    remaining_time="00:10:00"
else
    job_id=$SLURM_JOB_ID
    remaining_time=$(squeue -h -j $job_id -o %L)
fi


num_threads=32

export npm_config_cache=$npm_cache_dir

echo "using npm cache dir: $npm_cache_dir"

# $(hostname)
# REGISTRY_HOST=pinckney2.vpc.ripley.cloud \

TOKIO_WORKER_THREADS=$num_threads \
REGISTRY_HOST=pinckney2.vpc.ripley.cloud \
NODE_NAME=$(hostname) \
MAX_JOB_TIME=$remaining_time \
cargo run --release
# ./target/release/historic_solver_job

rm -rf $npm_cache_dir