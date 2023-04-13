#!/bin/bash

# change these to be your own users
FOLLOWER_USER="federico"

# change to increase/decrease number of workers
NUM_XFER_WORKERS=2
NUM_COMP_WORKERS=0

export PATH="$PATH:/home/$FOLLOWER_USER/.cargo/bin/"
source "/home/$FOLLOWER_USER/.cargo/env"
cd /home/$FOLLOWER_USER/npm-follower/blob_idx_server/
sudo -u $FOLLOWER_USER cargo run --release -- $NUM_COMP_WORKERS $NUM_XFER_WORKERS
