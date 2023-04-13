#!/bin/bash
set -e

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

echo "Building database_exporting binary..."
pushd $SCRIPT_DIR
~/.cargo/bin/cargo install --path . --root . --force
rm .crates.toml
rm .crates2.json
popd

echo "Starting export of external metadata."
# tmux new-session -d -s export_metadata_external_session "sudo su -c \"$SCRIPT_DIR/bin/database_exporting\" postgres"

cd $SCRIPT_DIR
sudo su -c "$SCRIPT_DIR/bin/database_exporting" postgres