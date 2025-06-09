# Immich Album Sync 
A CLI tool to sync a single Immich album to a local folder on a schedule.

## Features
- Uses Immich API to fetch the exact list of assets in an album
- Downloads missing assets only
- Remembers already-synced assets in `sync-history.json`
- Can be run manually or as a Windows Scheduled Task at boot

## Setup
1. **Copy your `config.json` from the old project or use the provided example.**
2. **Build the project:**
   ```shell
   cargo build --release
   ```
3. **Run the sync manually:**
   ```shell
   cargo run --release
   ```
4. **Set up the scheduled task:**
   ```shell
   cargo run --release --bin setup_schtask
   ```
   Or build and run `setup_schtask.exe` directly.

## Requirements
- Rust (https://rustup.rs)
- Immich server with API access

## Notes
- The tool will create `sync-history.json` and your destination folder if they do not exist.
- If you use an album name instead of a UUID, you must resolve the UUID yourself (current version expects UUID).
- To remove the scheduled task, run:
  ```shell
  schtasks /Delete /TN ImmichAlbumSync /F
  ```

## License
MIT
