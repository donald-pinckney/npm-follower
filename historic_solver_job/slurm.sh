#!/bin/bash
#SBATCH --nodes=1
#SBATCH --mem=16G
#SBATCH --export=ALL
#SBATCH --cpus-per-task=24
#SBATCH --time=24:00:00
#SBATCH --job-name=historic_solver_job
#SBATCH --partition=short

ssh -L 5431:localhost:5432  pinckney@pinckney2.vpc.ripley.cloud -N -f
module unload discovery
./launch.sh
