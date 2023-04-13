#!/bin/bash

# change these to be your own users
FOLLOWER_USER="federico"

export PATH="$PATH:/home/$FOLLOWER_USER/.cargo/bin/"
source "/home/$FOLLOWER_USER/.cargo/env"
cd /home/$FOLLOWER_USER/npm-follower/downloader
sudo -u $FOLLOWER_USER ./continuous_script.sh cluster 2
