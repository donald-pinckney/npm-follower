FROM ubuntu:focal

ARG DEBIAN_FRONTEND=noninteractive
ENV TZ=Etc/UTC

RUN apt-get update && apt-get install -y vim curl libcurl4-openssl-dev libxml2-dev libssl-dev ca-certificates gnupg wget lsb-release r-base r-base-dev && apt-get clean all

RUN sh -c 'echo "deb http://apt.postgresql.org/pub/repos/apt $(lsb_release -cs)-pgdg main" > /etc/apt/sources.list.d/pgdg.list'
RUN wget --quiet -O - https://www.postgresql.org/media/keys/ACCC4CF8.asc | apt-key add -
RUN apt-get update
RUN apt-get -y install postgresql libpq-dev

ADD --chown=postgres postgresql.conf /etc/postgresql/15/main/postgresql.conf
ADD --chown=postgres pg_hba.conf /etc/postgresql/15/main/pg_hba.conf

COPY npm-follower-anon /npm-follower-anon
COPY plotting_scripts /plotting_scripts
COPY r_packages.r /r_packages.r

RUN mkdir -p /plots/general
RUN mkdir -p /plots/rq1
RUN mkdir -p /plots/rq2
RUN mkdir -p /plots/rq3
RUN mkdir -p /plots/rq4

RUN Rscript /r_packages.r


RUN service postgresql start && su -c 'psql -c "create database npm_data;"' postgres && service postgresql stop

COPY --chown=postgres npm-follower-postgres-analysis-dump-latest/ /npm-follower-postgres-analysis-dump-latest/
RUN service postgresql start && su -c 'pg_restore -d npm_data -e -O -j 4 /npm-follower-postgres-analysis-dump-latest/ && psql -c "checkpoint;"' postgres && sleep 300 && service postgresql stop

RUN service postgresql start && su -c 'psql -d npm_data -c "create user data_analyzer; grant select on all tables in schema public to data_analyzer; grant select on all tables in schema analysis to data_analyzer; grant usage on schema public to data_analyzer; grant usage on schema analysis to data_analyzer;"' postgres && service postgresql stop