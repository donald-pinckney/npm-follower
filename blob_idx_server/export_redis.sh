#!/bin/bash

# this script exports the entirety of the redis database to a zip file.
# it does the following things:
# 1. stop the cluster-downloader service
# 2. stop the job-scheduler service
# 3. run the save command on the redis server
# 4. stop the redis server
# 5. zip the dump.rdb and appendonly.aof files
# 6. start the redis server
# 7. start the job-scheduler service
# 8. start the cluster-downloader service


# check if running as root
if [ "$EUID" -ne 0 ]
  then echo "Please run as root"
  exit
fi

echo "1/8) stopping cluster-downloader service"
systemctl stop cluster-downloader

echo "2/8) stopping job-scheduler service"
systemctl stop job-scheduler

echo "3/8) running redis save command"
redis-cli save

echo "4/8) stopping redis server"
systemctl stop redis

echo "5/8) zipping redis files"
zip -r redis.zip /var/lib/redis/dump.rdb /var/lib/redis/appendonly.aof

echo "6/8) starting redis server"
systemctl start redis

echo "7/8) starting job-scheduler service"
systemctl start job-scheduler

echo "8/8) starting cluster-downloader service"
systemctl start cluster-downloader
