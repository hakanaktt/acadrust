### Cross-library line test: acadrust vs ACadSharp
### Writes a simple line DWG with both libraries for each version,
### then cross-reads the other library's output.
###
### Usage:  .\scripts\cross_line_test.ps1

$ErrorActionPreference = "Stop"

$root      = "test_output/cross_line_test"
$rustOut   = "$root/acadrust"
$sharpOut  = "$root/acadsharp"

Write-Host "============================================================"
Write-Host "  Cross-Library Line Test: acadrust vs ACadSharp"
Write-Host "============================================================"
Write-Host ""

# ── Step 1: Write simple line DWGs with acadrust ──
Write-Host "[1/4] acadrust: write-lines -> $rustOut"
cargo run --bin cross_line_test -- write-lines $rustOut
Write-Host ""

# ── Step 2: Write simple line DWGs with ACadSharp ──
Write-Host "[2/4] ACadSharp: write-lines -> $sharpOut"
dotnet run --project ACadSharp/src/ACadSharp.Examples/ACadSharp.Examples.csproj -- write-lines $sharpOut
Write-Host ""

# ── Step 3: acadrust reads ACadSharp DWGs ──
Write-Host "[3/4] acadrust cross-reads ACadSharp DWGs"
cargo run --bin cross_line_test -- cross-read $sharpOut
Write-Host ""

# ── Step 4: ACadSharp reads acadrust DWGs ──
Write-Host "[4/4] ACadSharp cross-reads acadrust DWGs"
dotnet run --project ACadSharp/src/ACadSharp.Examples/ACadSharp.Examples.csproj -- cross-read $rustOut
Write-Host ""

Write-Host "============================================================"
Write-Host "  Done! Output directories:"
Write-Host "    acadrust DWGs:  $rustOut"
Write-Host "    ACadSharp DWGs: $sharpOut"
Write-Host "============================================================"
