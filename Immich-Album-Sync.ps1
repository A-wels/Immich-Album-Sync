<#
.SYNOPSIS
    Sync one Immich album to a local folder.

.DESCRIPTION
    • Uses  GET /api/albums/{albumId}?withoutAssets=false
      to pull the exact list of assets.
    • Downloads missing assets with
      GET /api/assets/{assetId}/original.
    • Remembers already-synced IDs in sync-history.json.

.NOTES
    File   : Immich-Album-Sync.ps1
    Needs  : PowerShell 5.1+
    Updated: 2025-05-16
#>

# --------------------------------------------------------------------------
# config & housekeeping
# --------------------------------------------------------------------------
$scriptPath = Split-Path -Parent $MyInvocation.MyCommand.Path
$config     = Get-Content (Join-Path $scriptPath 'config.json') -Raw | ConvertFrom-Json

foreach ($prop in 'ApiUrl','ApiKey','AlbumId','LocalFolder','IntervalMinutes') {
    if (-not $config.$prop) { throw "config.json is missing '$prop'." }
}

# trim accidental white-space
$config.AlbumId = ($config.AlbumId -as [string]).Trim()

# guarantee base URL ends with /api
if ($config.ApiUrl.TrimEnd('/') -notmatch '/api$') {
    $config.ApiUrl = $config.ApiUrl.TrimEnd('/') + '/api'
}

# create destination & history DB
$null  = New-Item -Path $config.LocalFolder -ItemType Directory -Force
$dbPath = Join-Path $scriptPath 'sync-history.json'
if (-not (Test-Path $dbPath)) {
    @{ LastSync = $null; ProcessedAssets = @() } |
        ConvertTo-Json | Out-File $dbPath
}
$syncHistory = Get-Content $dbPath -Raw | ConvertFrom-Json

# --------------------------------------------------------------------------
# helpers
# --------------------------------------------------------------------------
function Invoke-ImmichApi {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [string]$Endpoint,

        [ValidateSet('GET','POST','PUT','PATCH','DELETE')]
        [string]$Method = 'GET',

        # optional query hashtable, e.g. @{ withoutAssets = $false }
        [hashtable]$Query,

        [object]$Body
    )

    # build URI safely, honouring query parameters
    $uri = '{0}/{1}' -f $config.ApiUrl.TrimEnd('/'), $Endpoint.TrimStart('/')
    if ($Query) {
        $qs = ($Query.GetEnumerator() | ForEach-Object {
            # convert Boolean to lower-case string accepted by Immich
            $val = $_.Value
            if ($val -is [bool]) { $val = $val.ToString().ToLower() }
            '{0}={1}' -f [System.Net.WebUtility]::UrlEncode($_.Name),
                           [System.Net.WebUtility]::UrlEncode($val)
        }) -join '&'
        $uri = "${uri}?$qs"
    }

    $params = @{
        Uri         = $uri
        Method      = $Method
        Headers     = @{ 'x-api-key' = $config.ApiKey }
        ContentType = 'application/json'
        ErrorAction = 'Stop'
    }

    if ($Method -ne 'GET' -and $Body) {
        $params.Body = ($Body | ConvertTo-Json -Depth 8)
    }

    Invoke-RestMethod @params
}

function Is-UUID ($v) { $v -match '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$' }

function Resolve-AlbumId {
    if (Is-UUID $config.AlbumId) { return $config.AlbumId }

    Write-Host "AlbumId '$($config.AlbumId)' is not a UUID, searching by name..."
    $albums = Invoke-ImmichApi -Endpoint 'albums'
    $match  = $albums | Where-Object { $_.albumName -eq $config.AlbumId }

    if (-not $match)        { throw "Album named '$($config.AlbumId)' not found." }
    if ($match.Count -gt 1) { throw "Album name not unique, specify the UUID." }

    return $match.id
}

$AlbumUuid = Resolve-AlbumId

function Get-AlbumWithAssets {
    Invoke-ImmichApi -Endpoint "albums/$AlbumUuid" -Query @{ withoutAssets = $false }
}

function Download-Asset {
    param(
        [Parameter(Mandatory)]
        $AssetId,
        [Parameter(Mandatory)]
        $Target
    )
    $uri = '{0}/assets/{1}/original' -f $config.ApiUrl.TrimEnd('/'), $AssetId
    try {
        Invoke-WebRequest -Uri $uri -Headers @{ 'x-api-key' = $config.ApiKey } `
                          -OutFile $Target -ErrorAction Stop
        $true
    } catch {
        Write-Warning "Download failed for ${AssetId}: $_"
        $false
    }
}

# --------------------------------------------------------------------------
# sync routine
# --------------------------------------------------------------------------
function Sync-Album {
    $album   = Get-AlbumWithAssets
    $assets  = $album.assets
    Write-Host ("[{0}] Album '{1}' - {2} assets" -f (Get-Date), $album.albumName, $assets.Count)

    $new = $skip = $fail = 0
    foreach ($a in $assets) {
        if ($syncHistory.ProcessedAssets -contains $a.id) { $skip++; continue }

        $ext  = [IO.Path]::GetExtension($a.originalPath); if (-not $ext) { $ext = '.jpg' }
        $file = Join-Path $config.LocalFolder "$($a.id)$ext"

        if (Download-Asset $a.id $file) {
            $syncHistory.ProcessedAssets += $a.id
            $new++
        } else { $fail++ }
    }

    $syncHistory.LastSync = (Get-Date).ToString('o')
    $syncHistory | ConvertTo-Json | Out-File $dbPath

    Write-Host "Finished - new:$new  skipped:$skip  failed:$fail"
}

# --------------------------------------------------------------------------
# main loop
# --------------------------------------------------------------------------
Write-Host 'Immich Album Sync - press Ctrl+C to stop.'
while ($true) {
    Sync-Album
    Start-Sleep -Seconds ($config.IntervalMinutes * 60)
}
