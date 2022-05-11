#!/bin/bash
#SBATCH --nodes=1
#SBATCH --time=24:00:00
#SBATCH --job-name=npm_db
#SBATCH --partition=short
#SBATCH --mem=64G
#SBATCH --exclusive
#SBATCH --mail-user=$USER@northeastern.edu
#SBATCH --mail-type=ALL
#SBATCH -N 1
#SBATCH -n 1

rm npm_db.sqlite3 npm_db.sqlite3-journal
time cargo run --release > output.log 2> error.log

echo "DONE (or failed)"
