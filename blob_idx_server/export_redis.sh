#!/bin/bash

# this script exports the entirety of the redis database to a zip file,
# keeping all services balanced and running.
# it does the following things:
# 1. stop the cluster-downloader service
# 2. stop the job-scheduler service
# 3. run redis "CONFIG SET auto-aof-rewrite-percentage 0" command
# 4. check redis "INFO persistence" command to make sure it's not rewriting or saving.
#    if it is, wait 5 seconds and check again until it's not.
# 5. run redis "SAVE" command
# 6. copy the redis db files to a zip file
# 7. restore the auto-aof-rewrite-percentage to 100
# 8. start the job-scheduler service
# 9. start the cluster-downloader service


# check if running as root
if [ "$EUID" -ne 0 ]
  then echo "Please run as root"
  exit
fi

echo "1/9) stopping cluster-downloader service"
systemctl stop cluster-downloader

echo "2/9) stopping job-scheduler service"
systemctl stop job-scheduler

echo "3/9) disabling auto-aof-rewrite-percentage"
redis-cli CONFIG SET auto-aof-rewrite-percentage 0

echo "4/9) waiting for redis to finish saving or rewriting"
while true; do
    if [[ $(redis-cli INFO persistence | grep -c "aof_rewrite_in_progress:0") -eq 1 && $(redis-cli INFO persistence | grep -c "rdb_bgsave_in_progress:0") -eq 1 ]]; then
        break
    fi
    echo -n "."
    sleep 5
done

echo "5/9) saving redis database"
redis-cli SAVE

echo "6/9) copying redis db files to zip file"
zip -r redis.zip /var/lib/redis/dump.rdb /var/lib/redis/appendonlydir/
chown redis:redis redis.zip

echo "7/9) restoring auto-aof-rewrite-percentage"
redis-cli CONFIG SET auto-aof-rewrite-percentage 100

echo "8/9) starting job-scheduler service"
systemctl start job-scheduler

echo "9/9) starting cluster-downloader service"
systemctl start cluster-downloader

echo "DONE"
