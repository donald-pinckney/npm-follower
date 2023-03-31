#!/bin/bash
set -e

echo "Starting export of external metadata."

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
tmux new-session -d -s export_metadata_external_session "sudo su -c \"bash $SCRIPT_DIR/_impl_export_metadata_external.sh\" postgres"
# sudo su -c "bash $SCRIPT_DIR/_impl_export_metadata_external.sh" postgres