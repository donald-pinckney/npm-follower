#!/bin/bash
set -e

if [[ "$(whoami)" != "postgres" ]]
then
    echo "This script must be run as the postgres user."
    exit 1
fi

tmp_backup_dir=/var/lib/postgresql/exports-npm-follower/metadata_external-tmp
local_backup_dir=/var/lib/postgresql/exports-npm-follower/metadata_external
cur_time=$(date -u --iso-8601=seconds)

echo "Starting export of external metadata to: $tmp_backup_dir/$cur_time/"
table_params=('-t' '__diesel_schema_migrations' '-t' 'dependencies' '-t' 'downloaded_tarballs' '-t' 'ghsa' '-t' 'packages' '-t' 'versions' '-t' 'vulnerabilities')
pg_dump -j 2 -F d -f $tmp_backup_dir/$cur_time/ "${table_params[@]}" --no-acl npm_data

echo "Creating tar file of dump"
tar cvf "$tmp_backup_dir/$cur_time.tar" "$tmp_backup_dir/$cur_time/"
chmod g-w "$tmp_backup_dir/$cur_time.tar"

echo "Cleaning up dump dir"
rm -rf "$tmp_backup_dir/$cur_time/"

echo "Moving tar file to final location"
mv "$tmp_backup_dir/$cur_time.tar" "$local_backup_dir/$cur_time.tar"

echo "Creating symlink to latest backup"
ln -sf "$cur_time.tar" "$local_backup_dir/latest.tar"


echo "Completed export of external metadata. Total size:"
du -Lh "$local_backup_dir/latest.tar"
