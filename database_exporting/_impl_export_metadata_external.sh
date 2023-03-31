#!/bin/bash
set -e

if [[ "$(whoami)" != "postgres" ]]
then
    echo "This script must be run as the postgres user."
    exit 1
fi

local_backup_dir=/var/lib/postgresql/exports-npm-follower/metadata_external
cur_time=$(date -u --iso-8601=seconds)

echo "Starting export of external metadata to: $local_backup_dir/$cur_time/"

table_params=('-t' '__diesel_schema_migrations' '-t' 'dependencies' '-t' 'downloaded_tarballs' '-t' 'ghsa' '-t' 'packages' '-t' 'versions' '-t' 'vulnerabilities')


pg_dump -j 2 -F d -f $local_backup_dir/$cur_time/ "${table_params[@]}" --no-acl npm_data

tar cvf "$local_backup_dir/$cur_time.tar" "$local_backup_dir/$cur_time/"
rm -rf "$local_backup_dir/$cur_time/"
chmod g-w "$local_backup_dir/$cur_time.tar"
ln -sf "$cur_time.tar" "$local_backup_dir/latest.tar"


echo "Completed export of external metadata. Total size:"
du -Lh "$local_backup_dir/latest.tar"
