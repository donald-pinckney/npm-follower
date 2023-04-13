#!/bin/bash

# change these to be your own users
FOLLOWER_USER="federico"

cd /home/$FOLLOWER_USER/npm-follower/downloader
sudo -u $FOLLOWER_USER ./continuous_script.sh cluster 2
