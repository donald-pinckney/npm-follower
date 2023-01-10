#!/bin/bash
#SBATCH --nodes=1
#SBATCH --mem=8G
#SBATCH --export=ALL
#SBATCH --cpus-per-task=24
#SBATCH --time=24:00:00
#SBATCH --job-name=historic_solver_job
#SBATCH --partition=short
#SBATCH --constraint=haswell|broadwell|skylake_avx512|zen2|zen|cascadelake

./launch.sh
