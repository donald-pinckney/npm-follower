#!/bin/bash

mkdir -p plotting_scripts/
for f in ../analysis/notebooks/*.ipynb; do
    echo $f
    jupyter nbconvert --output-dir=plotting_scripts/ --to script $f
done

sudo docker build -t artifact .
# sudo docker run -it artifact bash
