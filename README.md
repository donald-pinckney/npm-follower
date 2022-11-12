# NPM Scraping Scripts

[![Rust](https://github.com/donald-pinckney/npm-follower/actions/workflows/rust.yml/badge.svg)](https://github.com/donald-pinckney/npm-follower/actions/workflows/rust.yml)

[DASHBOARD](https://grafana.apps.in.ripley.cloud/public-dashboards/23a7905953e5448f9e2732a75e9d01f0)

## Prerequisites

### Available Disk Space

The PostgreSQL database will (currently) need around 150 GB total.

### macOS Only:

```bash
brew install libpq
```

### Rust & Cargo

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Diesel CLI

Install the Diesel CLI with:

```bash
cargo install diesel_cli --no-default-features --features postgres
```


### PostgreSQL

1. Download and install PostgreSQL as appropriate for your system.
2. Configure the port to 5431 (default is 5432!)



## Database Setup

Next, we need to initialze the database. Run:

```bash
pushd postgres_db
diesel setup
popd
```


## InfluxDB Setup

You may choose to either enable and setup InfluxDB for logging, or disable it.

### Enabling and Setting up InfluxDB

If you want to enable InfluxDB, you need to set `ENABLE_INFLUX_DB_LOGGING=true` and configure `INFLUX_DB_URL`, `INFLUX_DB_ORG`, `INFLUX_DB_BUCKET` in `.env`.
Then you need to set your API token in `.secret.env`:

```bash
echo "export INFLUX_DB_TOKEN=<TYPE API TOKEN HERE>" > .secret.env
```

### Disabling InfluxDB

If you don't want to use InfluxDB logging, just disable it in `.env` by setting `ENABLE_INFLUX_DB_LOGGING=false`.

### Configuring Telegraf

First, make sure `telegraf` has been started at least once:

```bash
systemctl start telegraf
```

Then, edit it:

```bash
systemctl edit telegraf
```

with the following contents:

```conf
[Service]
EnvironmentFile=<PATH TO YOUR REPO CLONE>/.env
EnvironmentFile=<PATH TO YOUR REPO CLONE>/.secret.env
```

Finally, restart Telegraf:

```bash
sudo systemctl daemon-reload
pushd services; ./restart_telegraf.sh; popd
```

## Running the Scripts

### NPM Changes Follower

The NPM Changes Follower script continually fetches changes from NPM, and insertes them into the `change_log` Postgres table.
It will fetch changes starting after the most recently fetched changes, so in case of crashing / server reboots, etc. it can be restarted without worry.
**TODO: currently the NPM Changes Follower will quit after a long enough delay of not receiving changes. So you really should run it in a loop, but I haven't automated that yet.**

To run the NPM Changes Follower, from this directory run:

```bash
cargo run --release --bin changes_fetcher
```

This will both build and run the NPM Changes Follower, in release mode. Once the follower catches up to present-day (maybe 12-48 hours), there should be somewhere near
~3 million rows (~100 GB).


### Download parser / queuer

Unlike the NPM Changes Follower, the Download Queuer does not run continually. Instead, upon each execution, 
the Download Queuer will scan the `change_log` table (populated by the NPM Changes Follower), 
and insert into the `download_tasks` table any tarball URLs that haven't already been added.

Running this for the **first time** with a fully-populated `change_log` table will take around 12 hours. 
After that, how long it takes to run depends on how often you run it, but should be pretty fast.
Running say every 10 minutes should be fine.
**TODO: Running the Download Querer on a schedule is not automated yet!** 

To run the Download Queuer, from this directory run:

```bash
cargo run --release --bin download_queuer
```

After all present-day tarballs have been inserted into the `download_tasks` table, there should be around ~25 million rows (~28 GB).

