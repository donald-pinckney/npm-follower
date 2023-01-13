#!/bin/bash

#mkdir /dev/shm/$JOB_ID
#npm_cache_dir="/dev/shm/$JOB_ID"

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


module load discovery
../target/release/historic_npm_registry &

sleep 5

module unload discovery

curl http://127.0.0.1:8372/now/react > /tmp/react_result.json

if grep -q '2015-10-28T21:36:14.876Z' /tmp/react_result.json; then
    echo "server seems ok"
else
    echo "server seems not ok!"
    exit 1
fi

# $(hostname)
# REGISTRY_HOST=pinckney2.vpc.ripley.cloud \

TOKIO_WORKER_THREADS=$num_threads \
REGISTRY_HOST=127.0.0.1:8372 \
NODE_NAME=$(hostname) \
MAX_JOB_TIME=$remaining_time \
../target/release/historic_solver_job

# rm -rf $npm_cache_dir
