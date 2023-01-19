# Artifact for A Large Scale Analysis of Semantic Versioning in NPM

## Overview of What is Included in the Artifact

This artifact contains the following:

- This README file with instructions for reproducing the plots in the paper.
- A Dockerfile to build an image containing a dump of our Postgres database, and the R scripts used to generate the plots in the paper.
- The Postgres database only contains package metadata, and not the full package tarballs, as we do not currently know how to distribute 19TB easily and anonymously.
- In addition, to make the artifact download smaller, we have removed a handful of large tables from the database that are not used in the analysis in the paper, and are only used by the underlying scraping system.
- `npm-follower-anon/`: an anonymized clone of the repository, which contains the full code for the system used to collect the data in the paper.

The Postgres database is structured in two parts:

- The `public` schema (the default schema) contains the tables that are the product of scraping, and the product of very compute heavy jobs that are very time-consuming to recompute.
- The `analysis` schema contains tables that have been derived, primarily using SQL scripts in `npm-follower-anon/analysis/scripts/`.

## Instructions for Reproducing the Plots in the Paper

### 0. Prerequisites

- Docker
- 500 GB of free disk space

### 1. Build the Docker image

In this directory, run:

```bash
# You made need to prepend `sudo` to docker commands, depending on your system
docker build -t artifact-npm-follower-image .
```

This command will take several hours to run, as it must install all dependencies for R, copy the compressed database dump into the image, and then restore the database.

### 2. Run and Login to the Docker image

Once you've built the Docker image, you can run it and login to it with the following commands:

```bash
# You made need to prepend `sudo` to docker commands, depending on your system
docker run -it -d --name artifact-npm-follower-container artifact-npm-follower-image bash
docker exec -it artifact-npm-follower-container /bin/bash
```

You should now be in a shell inside the Docker container.

### 3. Start Postgres

```bash
service postgresql start
```

Once it responds with `[ OK ]`, then proceed to the next step.

### 4. Test database connection

Before proceeding, briefly check that the database is running correctly and is accepting connections.
Test this by running the following command:

```bash
psql -U data_analyzer npm_data -c "\dt"
```

Once you see a response listing the tables in the database, then you know that Postgres is ready to go,
and you can proceed to the next step.

### 5. Recreate the plots

```bash
cd /
Rscript plotting_scripts/rq1.r # this will create plots inside /plots/general
Rscript plotting_scripts/rq1.r # this will create plots inside /plots/rq1
Rscript plotting_scripts/rq2.r # this will create plots inside /plots/rq2
Rscript plotting_scripts/rq3_a.r # this will create plots inside /plots/rq3
Rscript plotting_scripts/rq3_b.r # this will create plots inside /plots/rq3
Rscript plotting_scripts/rq4.r # this will create plots inside /plots/rq4
```

Please note that each script takes a couple of minutes to run.

### 6. Copying Plots to the Host Machine to View

You may run a command like the following **on the host machine** to copy the plots directory to the host machine:

```bash
# You made need to prepend `sudo` to docker commands, depending on your system
docker cp artifact-npm-follower-container:/plots .
```

Then you may view the plots and compare to the plots in the paper.

### 7. Stopping the Docker container

In a shell on the host machine, run:

```bash
# You made need to prepend `sudo` to docker commands, depending on your system
docker stop artifact-npm-follower-container
```


