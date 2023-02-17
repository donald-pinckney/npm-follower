# Historic NPM Registry

When investigating historical trends in OSS, an interesting question to ask is "how *would* a package's dependencies have been solved at X point in time".

This is a simple tool to do that, deployable as a custom NPM registry, which simply forwards requests to the NPM registry and re-writes results to use the versions of packages that were available at the time.

## Usage

1. Start the server in this directory: `cargo run --release`. It will listen on port 80.
2. Use the `npm` CLI just as you normally would, but with the flag: `--registry http://SERVER_ADDRESS/TIME/`
where `TIME` is a URL-encoded RFC3339 timestamp. For example:
```bash
npm install --registry http://localhost:80/2022-01-03%2019%3A39%3A20.045534134%20UTC/
```