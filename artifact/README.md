# Artifact for A Large Scale Analysis of Semantic Versioning in NPM

## 1. Import the Docker image

## 2. Run and Login to the Docker image

```bash
# You made need to prepend `sudo` to the following commands, depending on your system
docker run -it -d --name artifact-npm-follower-container artifact-npm-follower-image bash
docker exec -it artifact-npm-follower-container /bin/bash
```

You should now be in a shell inside the Docker container.

## 3. Start Postgres

```bash
service postgresql start
```

Postgres may fail to start at first, and it may take a while to start.
If there is an error when running the above command, try running it again.
Once it responds with `[ OK ]`, then proceed to the next step.

## 4. Wait for Postgres to finish starting

Postgres may take some time (up to 15+ minutes) to start.
You can check the status of Postgres by running the following command:

```bash
psql -U data_analyzer npm_data -c "\dt"
```

Once you see a response listing the tables in the database, then you know that Postgres has finished starting,
and you can proceed to the next step.

## 5. Recreate the plots

```bash
cd /
Rscript plotting_scripts/rq1.r # this will create plots inside /plots/rq1
Rscript plotting_scripts/rq2.r # this will create plots inside /plots/rq2
Rscript plotting_scripts/rq3_a.r # this will create plots inside /plots/rq3
Rscript plotting_scripts/rq3_b.r # this will create plots inside /plots/rq3
Rscript plotting_scripts/rq4.r # this will create plots inside /plots/rq4
```


## 6. Exit the Docker container

In a shell on the host machine, run:

```bash
docker stop artifact-npm-follower-container
```


