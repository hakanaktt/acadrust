# Batch-audit generated DWG/DXF files in BricsCAD recovery mode.
#
# `_.AUDITCTL 1` makes RECOVER write a `<stem>.adt` report next to the source
# file, containing "Entities / Errors / Fixes" counts plus a per-object list of
# what failed validation. That report is the authoritative result; the session
# .log carries no audit summary in V20, so parsing the .adt is the reliable
# route.
#
# RECOVER (not OPEN) is used deliberately: it audits and reports rather than
# silently repairing, and it will not raise a modal prompt on a damaged file.
#
# Files are batched per launch because BricsCAD startup dominates per-file cost.

param(
    [string]$Root      = "D:\GitHub\acadrust\target\cad_validate",
    [string]$BricsCAD  = "D:\Bricsys\BricsCAD\bricscad.exe",
    [int]   $BatchSize = 8,
    [int]   $TimeoutMs = 600000
)

$ErrorActionPreference = "Continue"
$work = Join-Path $env:TEMP "cad_validate"
New-Item -ItemType Directory -Force -Path $work | Out-Null

$files = Get-ChildItem -Path $Root -Recurse -Include *.dwg, *.dxf | Sort-Object FullName
if (-not $files) { Write-Host "No files found under $Root"; exit 1 }
Write-Host "Auditing $($files.Count) files in batches of $BatchSize"

# Remove stale .adt reports so a missing report is distinguishable from an old one.
Get-ChildItem -Path $Root -Recurse -Filter *.adt -ErrorAction SilentlyContinue |
    Remove-Item -Force -ErrorAction SilentlyContinue

$batchNo = 0
for ($i = 0; $i -lt $files.Count; $i += $BatchSize) {
    $batch = $files[$i..([Math]::Min($i + $BatchSize - 1, $files.Count - 1))]
    $batchNo++

    $lines = @("_.FILEDIA 0", "_.AUDITCTL 1")
    foreach ($f in $batch) {
        $lines += "_.RECOVER"
        $lines += $f.FullName
        # Discard the recovered drawing; the .adt report is already on disk.
        $lines += "_.CLOSE"
        $lines += "_N"
    }
    $lines += "_.QUIT"
    $lines += "_N"

    $scr = Join-Path $work "batch_$batchNo.scr"
    Set-Content -Path $scr -Value ($lines -join "`r`n") -Encoding Ascii

    Write-Host "[batch $batchNo] $($batch.Count) files..." -NoNewline
    $p = Start-Process -FilePath $BricsCAD `
                       -ArgumentList '/nologo', '/b', $scr `
                       -PassThru -WindowStyle Minimized
    if ($p.WaitForExit($TimeoutMs)) {
        Write-Host " exited($($p.ExitCode))"
    } else {
        Write-Host " TIMEOUT - killing (modal dialog likely)"
        Get-Process -Name bricscad -ErrorAction SilentlyContinue | Stop-Process -Force
        Start-Sleep -Seconds 3
    }
}

# ── Parse the .adt reports ───────────────────────────────────────────
$results = foreach ($f in $files) {
    $adt = [System.IO.Path]::ChangeExtension($f.FullName, ".adt")
    if (-not (Test-Path $adt)) {
        [pscustomobject]@{
            File = $f.Name; Ext = $f.Extension.TrimStart('.')
            Entities = "-"; Errors = "NO_REPORT"; Fixes = "-"
            Solid3dError = ""; Removed = ""; Detail = "no .adt written (load failed or hung)"
        }
        continue
    }
    $c = Get-Content $adt -Raw
    $ent = [regex]::Match($c, "Entities\s*:\s*(\d+)").Groups[1].Value
    $err = [regex]::Match($c, "Errors\s*:\s*(\d+)").Groups[1].Value
    $fix = [regex]::Match($c, "Fixes\s*:\s*(\d+)").Groups[1].Value

    # Did a 3D solid / mesh fail, and was it discarded?
    $solidBlock = [regex]::Match($c, "Name\s*:\s*AcDb(3dSolid|PolyFaceMesh)[\s\S]*?(?=(\r?\nName\s*:)|$)")
    $solidErr = ""
    $removed = ""
    if ($solidBlock.Success) {
        $b = $solidBlock.Value
        # Condense the geometry complaints onto one line.
        $solidErr = (($b -split "`r?`n" |
            Where-Object { $_ -match "error|Error|failed|not connected|topology|backptr|multiple groups|without" } |
            ForEach-Object { $_.Trim() }) -join " / ")
        if ($b -match "Default\s*:\s*(.+)") { $removed = $Matches[1].Trim() }
    }

    # Everything that failed validation, by object type.
    $names = ([regex]::Matches($c, "Name\s*:\s*(\S+)") | ForEach-Object { $_.Groups[1].Value }) -join ","

    [pscustomobject]@{
        File = $f.Name; Ext = $f.Extension.TrimStart('.')
        Entities = $ent; Errors = $err; Fixes = $fix
        Solid3dError = $solidErr; Removed = $removed; Detail = $names
    }
}

Write-Host "`n================ RESULTS ================"
$results | Format-Table -AutoSize File, Ext, Entities, Errors, Fixes, Removed

$csv = Join-Path $Root "recover_results.csv"
$results | Export-Csv -Path $csv -NoTypeInformation
Write-Host "Wrote $csv"

$clean   = ($results | Where-Object { $_.Errors -eq "0" }).Count
$dirty   = ($results | Where-Object { $_.Errors -match '^\d+$' -and $_.Errors -ne "0" }).Count
$noreport= ($results | Where-Object { $_.Errors -notmatch '^\d+$' }).Count
$geomBad = ($results | Where-Object { $_.Solid3dError -ne "" }).Count
Write-Host "`nclean=$clean  with_errors=$dirty  no_report=$noreport  geometry_rejected=$geomBad  total=$($results.Count)"

if ($geomBad -gt 0) {
    Write-Host "`n---- files where the solid/mesh itself failed validation ----"
    $results | Where-Object { $_.Solid3dError -ne "" } | ForEach-Object {
        Write-Host "`n$($_.File)  [errors=$($_.Errors) action=$($_.Removed)]"
        Write-Host "   $($_.Solid3dError)"
    }
}
