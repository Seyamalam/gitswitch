# gitswitch installer — builds locally from source (no CI, no binary downloads).
# Usage (PowerShell):
#   irm https://raw.githubusercontent.com/Seyamalam/gitswitch/main/install.ps1 | iex
# Env:
#   $env:GITSWITCH_VERSION  branch/tag to build (default: main)
#   $env:GITSWITCH_SOURCE   local checkout to build instead of cloning (used for testing)
$ErrorActionPreference = "Stop"

$Repo = "Seyamalam/gitswitch"
$Version = if ($env:GITSWITCH_VERSION) { $env:GITSWITCH_VERSION } else { "main" }

if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
  throw "gitswitch: git is required but not installed. Run: winget install Git.Git"
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
  Write-Host "gitswitch: installing Rust toolchain (rustup) ..."
  $Rustup = Join-Path ([IO.Path]::GetTempPath()) "rustup-init.exe"
  Invoke-WebRequest -Uri "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe" -OutFile $Rustup
  & $Rustup -y --profile minimal --default-toolchain stable | Out-Null
  Remove-Item $Rustup -Force
  $CargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
  if ($env:Path -notlike "*$CargoBin*") { $env:Path = "$CargoBin;$env:Path" }
}

if ($env:GITSWITCH_SOURCE) {
  $Src = $env:GITSWITCH_SOURCE
} else {
  $Tmp = Join-Path ([IO.Path]::GetTempPath()) ([IO.Path]::GetRandomFileName())
  $Src = Join-Path $Tmp "gitswitch"
  git clone --depth 1 --branch $Version "https://github.com/$Repo.git" $Src
}

cargo install --locked --path $Src
& gitswitch --version
Write-Host "Installed. If 'gitswitch' is not found in a new terminal, add $env:USERPROFILE\.cargo\bin to your PATH."
