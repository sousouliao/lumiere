# Explicit fixture: back up and temporarily isolate only Lumiere installer keys.
# Never launch or uninstall the user's app. Persist backups for interrupted recovery.
$ErrorActionPreference = 'Stop'
$repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$fixture = Join-Path $repository 'artifacts\windows\installer-tests'
$backup = Join-Path $fixture 'registration-backup'
$version = (Get-Content (Join-Path $repository 'apps\desktop\package.json') -Raw -Encoding UTF8 | ConvertFrom-Json).version
$installer = Join-Path $repository "artifacts\windows\release\Lumiere-Setup-$version-x64.exe"
$legacy = Join-Path $repository 'artifacts\windows\build\win-unpacked'
$keys = @(
  'Software\Microsoft\Windows\CurrentVersion\Uninstall\25f85616-d352-524a-a5cc-01558c0b783c',
  'Software\Microsoft\Windows\CurrentVersion\Uninstall\Lumiere',
  'Software\Lumiere\Lumiere'
)
function Write-Json($path, $value) {
  [IO.File]::WriteAllText($path, ($value | ConvertTo-Json -Depth 10), (New-Object Text.UTF8Encoding($false)))
}
function Restore-Registration {
  $statePath = Join-Path $backup 'state.json'
  if (!(Test-Path -LiteralPath $statePath)) { return }
  $state = Get-Content -LiteralPath $statePath -Raw -Encoding UTF8 | ConvertFrom-Json
  if ($state.completed) { return }
  for ($i = 0; $i -lt $keys.Count; $i++) {
    $keyPath = "Registry::HKEY_CURRENT_USER\$($keys[$i])"
    if (Test-Path -LiteralPath $keyPath) { Remove-Item -LiteralPath $keyPath -Recurse -Force }
    if ($state.present[$i]) {
      & reg.exe import (Join-Path $backup "$i.reg") | Out-Null
      if ($LASTEXITCODE -ne 0) { throw 'Original registration restore failed' }
    }
  }
  $state.completed = $true
  Write-Json $statePath $state
}
function Clear-Registration {
  foreach ($key in $keys) {
    $path = "Registry::HKEY_CURRENT_USER\$key"
    if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Recurse -Force }
  }
}
function Run-Installer([string]$root, [bool]$explicitRoot = $false, [bool]$expectFailure = $false) {
  $arguments = '/S /NS' 
  if ($explicitRoot) { $arguments += " /D=$root" }
  $process = Start-Process -FilePath $installer -ArgumentList $arguments -WindowStyle Hidden -PassThru
  $null = $process.Handle
  if (!$process.WaitForExit(120000)) { throw 'Installer timed out; retain backups for recovery' }
  $process.Refresh()
  if ($expectFailure) {
    if ($process.ExitCode -eq 0) { throw 'Fault fixture unexpectedly succeeded' }
  } elseif ($process.ExitCode -ne 0) { throw "Installer failed ($($process.ExitCode)): $root" }
}
function Check-Payload([string]$root) {
  $manifest = Get-Content -LiteralPath (Join-Path $root '.lumiere-install.json') -Raw -Encoding UTF8 | ConvertFrom-Json
  foreach ($file in $manifest.files) {
    if ((Get-FileHash -LiteralPath (Join-Path $root $file.path) -Algorithm SHA256).Hash.ToLower() -ne $file.sha256) { throw 'Installed payload differs' }
  }
  $newKey = Get-ItemProperty -LiteralPath "Registry::HKEY_CURRENT_USER\$($keys[1])"
  if ($newKey.InstallLocation.Trim('"') -ne $root) { throw 'Installation location changed' }
  if (Test-Path -LiteralPath "Registry::HKEY_CURRENT_USER\$($keys[0])") { throw 'Duplicate legacy registration remains' }
  if (!(Test-Path -LiteralPath (Join-Path $root 'user notes.txt'))) { throw 'Unknown user file removed' }
}
function Legacy-Registration([string]$root) {
  $path = "Registry::HKEY_CURRENT_USER\$($keys[0])"
  New-Item -Path $path -Force | Out-Null
  New-ItemProperty -LiteralPath $path -Name DisplayName -Value 'Lumiere 0.6.0' -Force | Out-Null
  New-ItemProperty -LiteralPath $path -Name DisplayVersion -Value '0.6.0' -Force | Out-Null
  New-ItemProperty -LiteralPath $path -Name UninstallString -Value "`"$root\Uninstall Lumiere.exe`" /currentuser" -Force | Out-Null
}
New-Item -ItemType Directory -Path $fixture -Force | Out-Null
Restore-Registration
New-Item -ItemType Directory -Path $backup -Force | Out-Null
$present = @()
for ($i = 0; $i -lt $keys.Count; $i++) {
  $exists = Test-Path -LiteralPath "Registry::HKEY_CURRENT_USER\$($keys[$i])"
  $present += $exists
  if ($exists) {
    & reg.exe export "HKCU\$($keys[$i])" (Join-Path $backup "$i.reg") /y | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Registration backup failed' }
  }
}
Write-Json (Join-Path $backup 'state.json') @{completed=$false;present=$present}
$settings = Join-Path $env:APPDATA 'Lumiere\settings.json'
$settingsBefore = if (Test-Path -LiteralPath $settings) { (Get-FileHash -LiteralPath $settings -Algorithm SHA256).Hash } else { $null }
$results = @()
try {
  Clear-Registration
  $fresh = Join-Path $fixture ('fresh custom ' + [char]0x5b89 + [char]0x88c5)
  New-Item -ItemType Directory -Path $fresh -Force | Out-Null
  [IO.File]::WriteAllText((Join-Path $fresh 'user notes.txt'), 'retain me')
  Run-Installer $fresh $true
  Check-Payload $fresh
  $results += @{case='fresh-custom-unicode';status='passed'}
  Run-Installer $fresh
  Check-Payload $fresh
  $results += @{case='tauri-reinstall-inherited-location';status='passed'}

  $beforeUpdate = (Get-Item -LiteralPath (Join-Path $fresh '.lumiere-install.json')).LastWriteTimeUtc
  $env:LUMIERE_TEST_UPDATE_HANDOFF = $fresh
  try {
    $cargoArguments = 'test -p lumiere-desktop --features custom-protocol official_updater_accepts_signed_bytes_and_rejects_tampering -- --ignored --nocapture'
    $updateTest = Start-Process -FilePath 'cargo.exe' -ArgumentList $cargoArguments -WorkingDirectory $repository -WindowStyle Hidden -RedirectStandardOutput (Join-Path $fixture 'updater-handoff.stdout') -RedirectStandardError (Join-Path $fixture 'updater-handoff.stderr') -PassThru
    $null = $updateTest.Handle
    if (!$updateTest.WaitForExit(120000)) { throw 'Updater handoff test timed out' }
    $updateTest.Refresh()
    if ($updateTest.ExitCode -ne 0) { throw "Official updater handoff subprocess failed ($($updateTest.ExitCode))" }
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
      Start-Sleep -Milliseconds 100
      $updated = (Get-Item -LiteralPath (Join-Path $fresh '.lumiere-install.json')).LastWriteTimeUtc -gt $beforeUpdate
    } while (!$updated -and [DateTime]::UtcNow -lt $deadline)
    if (!$updated) { throw 'Official updater installer did not commit' }
    $hostPid = (Get-Content -LiteralPath (Join-Path $fresh 'fixture-host.json') -Raw -Encoding UTF8 | ConvertFrom-Json).hostPid
    if (Get-Process -Id $hostPid -ErrorAction SilentlyContinue) { throw 'Host survived updater handoff' }
    Check-Payload $fresh
    $results += @{case='official-updater-verified-download-host-exit-nsis-handoff';status='passed'}
  } finally { Remove-Item Env:LUMIERE_TEST_UPDATE_HANDOFF -ErrorAction SilentlyContinue }

  Clear-Registration
  $upgraded = Join-Path $fixture 'legacy custom'
  New-Item -ItemType Directory -Path $upgraded -Force | Out-Null
  $inventory = Get-Content -LiteralPath (Join-Path $repository 'apps\desktop\build\windows-installer\legacy-files.v0.6.0.json') -Raw -Encoding UTF8 | ConvertFrom-Json
  foreach ($file in $inventory.files) {
    $destination = Join-Path $upgraded $file
    New-Item -ItemType Directory -Path ([IO.Path]::GetDirectoryName($destination)) -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $legacy $file) -Destination $destination -Force
  }
  [IO.File]::WriteAllText((Join-Path $upgraded 'Uninstall Lumiere.exe'), 'never execute this')
  [IO.File]::WriteAllText((Join-Path $upgraded 'user notes.txt'), 'retain me')
  [IO.File]::WriteAllText((Join-Path $upgraded 'resources\user image.txt'), 'retain image')
  Legacy-Registration $upgraded
  $locked = [IO.File]::Open((Join-Path $upgraded 'chrome_100_percent.pak'), [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
  try { Run-Installer $upgraded $false $true } finally { $locked.Dispose() }
  Run-Installer $upgraded
  Check-Payload $upgraded
  foreach ($file in $inventory.files) {
    if ($file -ne 'Lumiere.exe' -and (Test-Path -LiteralPath (Join-Path $upgraded $file))) { throw "Legacy file remains: $file" }
  }
  if (!(Test-Path -LiteralPath (Join-Path $upgraded 'resources\user image.txt'))) { throw 'Unknown nested file removed' }
  if (Test-Path -LiteralPath (Join-Path $upgraded 'Uninstall Lumiere.exe')) { throw 'Legacy uninstaller remains' }
  $results += @{case='legacy-full-inventory-failure-recovery-inherited-location';status='passed'}
  Write-Json (Join-Path $fixture 'results.json') $results
} finally {
  Restore-Registration
  $settingsAfter = if (Test-Path -LiteralPath $settings) { (Get-FileHash -LiteralPath $settings -Algorithm SHA256).Hash } else { $null }
  if ($settingsBefore -ne $settingsAfter) { throw 'User settings bytes changed during fixture' }
}
$results | Format-Table -AutoSize
