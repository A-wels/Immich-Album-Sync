
# Immich Album Sync

A PowerShell script to sync a single Immich album to a local folder on a schedule.

## Features
- Uses Immich API to fetch the exact list of assets in an album
- Downloads missing assets only
- Remembers already-synced assets in `sync-history.json`
- Can be run manually or as a Windows Scheduled Task at boot

## How it works
- Fetches album assets with: `GET /api/albums/{albumId}?withoutAssets=false`
- Downloads each asset with: `GET /api/assets/{assetId}/original`
- Keeps a local sync history to avoid duplicate downloads

## Setup
1. **Clone or copy this folder to your PC.**
2. **Configure your settings:**
   - Copy `config.example.json` to `config.json` and fill in:
     - `ApiUrl`: Your Immich server API base URL (should end with `/api`)
     - `ApiKey`: Your Immich API key
     - `AlbumId`: Album UUID or name (if name, must be unique)
     - `LocalFolder`: Where to save images
     - `IntervalMinutes`: How often to sync
3. **Run the script manually:**
   ```powershell
   .\Immich-Album-Sync.ps1
   ```
   The script will run in a loop, syncing at the interval you set.

## Run at Startup (Windows Scheduled Task)
1. Open PowerShell as Administrator
2. Run:
   ```powershell
   .\setup-schtask.ps1
   ```
   This will create a scheduled task named `ImmichAlbumSync` that runs the sync script at every PC boot as SYSTEM.

## Notes
- The script will create `sync-history.json` and your destination folder if they do not exist.
- If you use an album name instead of a UUID, it must be unique.
- To remove the scheduled task, run:
  ```powershell
  Unregister-ScheduledTask -TaskName ImmichAlbumSync -Confirm:$false
  ```

## Requirements
- PowerShell 5.1+
- Immich server with API access

## Troubleshooting
- Make sure your `ApiUrl` ends with `/api` (e.g., `https://your-immich-server/api`)
- Check your API key and album ID/name are correct
- Run the script manually first to verify config and connectivity

## License
MIT
