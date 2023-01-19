#!/bin/bash
local_backup_dir=/etc/postgresql/15/main/npm-follower-postgres-analysis-dump-latest
rm -rf $local_backup_dir

echo "Starting local backup to: $local_backup_dir/"

table_exclusion=('-T' 'analysis.possible_install_deps' '-T' '__diesel_schema_migrations' '-T' 'change_log' '-T' 'diff_log' '-T' 'download_tasks' '-T' 'downloaded_tarballs' '-T' 'historic_solver_job_results_oldnesses_dont_separate_tracks' '-T' 'internal_diff_log_state' '-T' 'internal_state' '-T' 'temp')
pg_dump -j 4 -F d -f $local_backup_dir/ "${table_exclusion[@]}" --no-acl npm_data

