#!/bin/bash
#SBATCH --reservation=pinckney.d
#SBATCH --mem=0
#SBATCH --exclusive
#SBATCH --export=ALL

ssh -L 5431:localhost:5432  pinckney@pinckney2.vpc.ripley.cloud -N -f
./launch.sh
