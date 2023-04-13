#!/bin/bash

# change these to be your own users
FOLLOWER_USER="federico"
CARGO_PATH="/home/$FOLLOWER_USER/.cargo/bin/cargo"

# change to increase/decrease number of workers
NUM_XFER_WORKERS=2
NUM_COMP_WORKERS=0

cd /home/$FOLLOWER_USER/npm-follower/blob_idx_server/
sudo -u $FOLLOWER_USER $CARGO_PATH run --release -- $NUM_COMP_WORKERS $NUM_XFER_WORKERS
