#!/bin/bash

CHANGE_DIR=/mnt/data/donald/npm_data/change_history

mkdir -p $CHANGE_DIR


until node index.js --changes_path_root=$CHANGE_DIR

do
    echo ""
    echo "********************************************************************************"
    echo "Scraper failed. Waiting 120 seconds before trying again..."
    sleep 120
    echo "Starting scraper again."
    echo "********************************************************************************"
    echo ""
done
