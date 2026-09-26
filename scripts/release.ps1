param(
    [Parameter(Mandatory = $true)][string]$Version,
    [switch]$DryRun,
    [string]$NotesFile
)

$ErrorActionPreference = "Stop"

function Invoke-Step([string]$label, [scriptblock]$body) {
    Write-Host "`n==> $label" -ForegroundColor Cyan
    $ErrorActionPreference = "Continue"
    & $body
    $code = $LASTEXITCODE
    $ErrorActionPreference = "Stop"
    if ($code -ne 0) { throw "$label failed (exit code $code)" }
}

$repo = "kua8/Nectar"
$tag = "v$Version"
$root = Resolve-Path "$PSScriptRoot/.."
Set-Location $root

if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "Version must look like 1.3.1" }
if (-not $env:TAURI_SIGNING_PRIVATE_KEY) { throw "TAURI_SIGNING_PRIVATE_KEY is not set, so the update can't be signed." }

$versionFiles = @{
    "package.json"                            = (Get-Content package.json -Raw | ConvertFrom-Json).version
    "src-tauri/tauri.conf.json"               = (Get-Content src-tauri/tauri.conf.json -Raw | ConvertFrom-Json).version
    "src-tauri/Cargo.toml"                    = [regex]::Match((Get-Content src-tauri/Cargo.toml -Raw), '(?m)^version = "([^"]*)"').Groups[1].Value
    "installer-app/package.json"              = (Get-Content installer-app/package.json -Raw | ConvertFrom-Json).version
    "installer-app/src-tauri/tauri.conf.json" = (Get-Content installer-app/src-tauri/tauri.conf.json -Raw | ConvertFrom-Json).version
    "installer-app/src-tauri/Cargo.toml"      = [regex]::Match((Get-Content installer-app/src-tauri/Cargo.toml -Raw), '(?m)^version = "([^"]*)"').Groups[1].Value
}
$mismatched = $versionFiles.GetEnumerator() | Where-Object { $_.Value -ne $Version }
if ($mismatched) {
    $mismatched | ForEach-Object { Write-Host "  $($_.Key) is $($_.Value)" -ForegroundColor Yellow }
    throw "Version files don't match $Version. Run 'bun run bump $Version' and commit before releasing."
}

if (-not $DryRun) {
    if (-not (Get-Command gh -ErrorAction SilentlyContinue)) { throw "GitHub CLI not found. Install it with: winget install GitHub.cli, then run: gh auth login" }
    $ErrorActionPreference = "Continue"
    gh auth status *> $null
    $loggedIn = $LASTEXITCODE -eq 0
    $ErrorActionPreference = "Stop"
    if (-not $loggedIn) { throw "Not logged in to GitHub. Run: gh auth login" }
    if (git status --porcelain) { throw "Working tree has uncommitted changes. Commit them first so the tag matches what's built." }
}

$out = Join-Path $root "release"
if (Test-Path $out) { Remove-Item $out -Recurse -Force }
New-Item -ItemType Directory $out | Out-Null

if ($NotesFile) {
    $notes = Get-Content $NotesFile -Raw
} else {
    $notes = (git log -1 --pretty=format:"%b" --no-merges | Out-String).Trim()
    if (-not $notes) { $notes = (git log -1 --pretty=format:"%s" --no-merges | Out-String).Trim() }
    $notes = $notes -replace "(?m)^(What's New|Improvements|Changed|Bug Fixes|Other):?\s*$", '### $1'
    git fetch --tags --quiet
    $prev = git tag --sort=-version:refname | Where-Object { $_ -like "v*" -and $_ -ne $tag } | Select-Object -First 1
    if ($prev) { $notes += "`n`n**Full Changelog**: https://github.com/$repo/compare/$prev...$tag" }
}
Write-Host "`nRelease notes:`n$notes`n"

Invoke-Step "Installing frontend dependencies" { bun install }
Invoke-Step "Building Nectar (NSIS update installer)" { bun run tauri build --bundles nsis }

$exe = Join-Path $root "src-tauri/target/release/nectar.exe"
$setup = Join-Path $root "src-tauri/target/release/bundle/nsis/nectar_${Version}_x64-setup.exe"
foreach ($f in @($exe, $setup, "$setup.sig")) {
    if (-not (Test-Path $f)) { throw "Expected build output not found: $f" }
}

Write-Host "`n==> Packaging portable build" -ForegroundColor Cyan
$portable = "nectar-$Version-portable-windows-x64.zip"
Compress-Archive -Path $exe -DestinationPath (Join-Path $out $portable)

Write-Host "`n==> Packaging installer payload" -ForegroundColor Cyan
$payload = Join-Path $root "installer-app/src-tauri/payload.zip"
Compress-Archive -Path $exe -DestinationPath $payload -Force

Push-Location installer-app
try {
    Invoke-Step "Installing installer-app dependencies" { npm install }
    Invoke-Step "Building custom installer" { npm run tauri build -- --no-bundle }
} finally {
    Pop-Location
}
$installerName = "Nectar-Setup-$Version.exe"
Copy-Item (Join-Path $root "installer-app/src-tauri/target/release/nectar-installer.exe") (Join-Path $out $installerName)

Copy-Item $setup $out
$setupName = Split-Path $setup -Leaf
$platform = [ordered]@{
    signature = (Get-Content "$setup.sig" -Raw).Trim()
    url       = "https://github.com/$repo/releases/download/$tag/$setupName"
}
$manifest = [ordered]@{
    version   = $Version
    notes     = $notes
    pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    platforms = [ordered]@{
        "windows-x86_64"      = $platform
        "windows-x86_64-nsis" = $platform
    }
}
[IO.File]::WriteAllText((Join-Path $out "latest.json"), ($manifest | ConvertTo-Json -Depth 5), (New-Object Text.UTF8Encoding $false))
[IO.File]::WriteAllText((Join-Path $out "notes.md"), $notes, (New-Object Text.UTF8Encoding $false))

$assets = @("latest.json", $setupName, $portable, $installerName) | ForEach-Object { Join-Path $out $_ }
Write-Host "`nBuilt:" -ForegroundColor Green
$assets | ForEach-Object { Write-Host ("  {0}  ({1:N1} MB)" -f (Split-Path $_ -Leaf), ((Get-Item $_).Length / 1MB)) }

if ($DryRun) {
    Write-Host "`nDry run: nothing was pushed or published. Files are in $out" -ForegroundColor Yellow
    return
}

Invoke-Step "Pushing commit" { git push origin HEAD }

$ErrorActionPreference = "Continue"
gh release view $tag --repo $repo *> $null
$exists = $LASTEXITCODE -eq 0
$ErrorActionPreference = "Stop"
if ($exists) {
    Invoke-Step "Release $tag already exists, replacing its assets" { gh release upload $tag --repo $repo --clobber @assets }
} else {
    $sha = (git rev-parse HEAD).Trim()
    Invoke-Step "Creating draft release $tag" {
        gh release create $tag --repo $repo --target $sha --title "Nectar $tag" --notes-file (Join-Path $out "notes.md") --draft @assets
    }
    Invoke-Step "Publishing $tag" { gh release edit $tag --repo $repo --draft=false --latest }
}

Write-Host "`nReleased https://github.com/$repo/releases/tag/$tag" -ForegroundColor Green
