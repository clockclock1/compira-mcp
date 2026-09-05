# Build multi-arch Docker image on Windows (Docker Desktop + buildx).
# Usage:
#   .\scripts\docker-build.ps1
#   .\scripts\docker-build.ps1 -Multi -Push -Image ghcr.io/org/compira-mcp:latest
param(
  [string]$Image = "compira-mcp:local",
  [switch]$Multi,
  [switch]$Push
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $Root

if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
  throw "docker is required"
}

docker buildx version | Out-Null
$builder = "compira-builder"
$exists = docker buildx ls 2>$null | Select-String $builder
if (-not $exists) {
  docker buildx create --name $builder --driver docker-container --use | Out-Null
} else {
  docker buildx use $builder | Out-Null
}
docker buildx inspect --bootstrap | Out-Null

if ($Multi) {
  $platforms = "linux/amd64,linux/arm64"
  Write-Host "Building $Image for $platforms..."
  if ($Push) {
    docker buildx build --platform $platforms -t $Image --push .
  } else {
    Write-Host "Multi-arch without -Push only caches the build. Use -Push to publish."
    docker buildx build --platform $platforms -t $Image --output "type=image,name=$Image,push=false" .
  }
} else {
  $arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToLower()
  $platform = switch -Regex ($arch) {
    "arm64|aarch64" { "linux/arm64" }
    default { "linux/amd64" }
  }
  Write-Host "Building $Image for host platform $platform..."
  docker buildx build --platform $platform -t $Image --load .
}

Write-Host "Done: $Image"
