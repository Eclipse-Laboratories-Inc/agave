# eclipse-replay
A tool to re-execute and prove Eclipse blocks.

## Usage
Build the system using
```sh
cargo build
```

After which, use the following to download the latest snapshot from mainnet:
```
# List the most recent snapshots
./target/debug/reclipse snapshot list
# The above would list the snapshots as:
# Listing the latest snapshots...
# Last snapshots:
#	Full: 48925545
#	Incremental: 48926747

# Download the snapshot
./target/debug/reclipse snapshot download
```

This downloads into directory `$PWD/cache/snapshots/` a snapshot file for example
```
1.1G Feb 17 18:35 snapshot-48925545-XZmEaihJKkvQEwkj5ecsYVKpYLwp3bkrc4sgbyAVwY2.tar.zst
```

Then, replay the snapshot using:
```
./target/debug/reclipse replay --snapshot-slot 48925545
```

### Limits of open files
If you encounter the error `Too many open files`, you can increase the limit using:
```sh
ulimit -n 4096
```
