# Seed real sample components and clean E2E junk libraries
$ErrorActionPreference = "Stop"
$Base = "http://127.0.0.1:8080"
$SampleDir = Join-Path (Split-Path $PSScriptRoot -Parent) "samples\demo-ui"

$login = Invoke-RestMethod "$Base/api/auth/login" -Method POST -ContentType "application/json" `
  -Body '{"username":"admin","password":"admin123"}'
$auth = @{ Authorization = "Bearer $($login.token)" }

Write-Host "=== Cleanup empty / junk libraries ===" -ForegroundColor Cyan
$libs = @(Invoke-RestMethod "$Base/api/libraries" -Headers $auth)
foreach ($lib in $libs) {
  $junk = $false
  if ($lib.source_type -eq "git" -and $lib.repo_url -match "Hello-World") { $junk = $true }
  if ($lib.source_type -eq "fetch" -and $lib.component_count -eq 0) { $junk = $true }
  if ($lib.name -match "^(upload-test|upload-v2|git-test|git-hello|git-v2|ai-test|ai-v2)$") { $junk = $true }
  if ($junk) {
    Write-Host "  delete $($lib.name) ($($lib.id))"
    Invoke-RestMethod "$Base/api/libraries/$($lib.id)" -Method DELETE -Headers $auth | Out-Null
  }
}

Write-Host "=== Create Demo UI library + upload components ===" -ForegroundColor Cyan
$lib = Invoke-RestMethod "$Base/api/libraries" -Method POST -Headers $auth -ContentType "application/json" `
  -Body '{"name":"Demo UI 示例组件","source_type":"upload","rules":"prefer:Cmp"}'
Write-Host "  library id=$($lib.id)"

$files = Get-ChildItem $SampleDir -Filter "*.vue"
$formArgs = @(
  "-s", "-w", "`n%{http_code}",
  "-X", "POST", "$Base/api/libraries/$($lib.id)/upload",
  "-H", "Authorization: Bearer $($login.token)",
  "-F", "use_ai=false",
  "-F", "auto_name=false"
)
foreach ($f in $files) {
  $formArgs += "-F"
  $formArgs += "files=@$($f.FullName)"
}
$upRaw = & curl.exe @formArgs
$lines = $upRaw -split "`n"
$code = $lines[-1]
$body = ($lines[0..($lines.Length - 2)] -join "`n") | ConvertFrom-Json
Write-Host "  upload http=$code task=$($body.task_id) files=$($files.Count)"

$task = $null
for ($i = 0; $i -lt 30; $i++) {
  Start-Sleep -Seconds 1
  $task = Invoke-RestMethod "$Base/api/tasks/$($body.task_id)" -Headers $auth
  if ($task.status -in @("completed", "failed")) { break }
}
Write-Host "  ingest: $($task.status) $($task.message)"

$comps = @(Invoke-RestMethod "$Base/api/libraries/$($lib.id)/components" -Headers $auth)
Write-Host "=== Components in Demo UI ($($comps.Count)) ===" -ForegroundColor Green
$comps | ForEach-Object { Write-Host "  - $($_.name)  $($_.file_path)  props=$($_.props.Count)" }

$stats = Invoke-RestMethod "$Base/api/stats" -Headers $auth
Write-Host "=== Stats: libraries=$($stats.libraries) components=$($stats.components) ===" -ForegroundColor Cyan

# Optional: AI fetch one real Element Plus component if LLM enabled
$ai = Invoke-RestMethod "$Base/api/ai/status" -Headers $auth
if ($ai.enabled) {
  Write-Host "=== AI fetch Element Plus Button (may take a while) ===" -ForegroundColor Cyan
  $aiLib = Invoke-RestMethod "$Base/api/libraries" -Method POST -Headers $auth -ContentType "application/json" `
    -Body '{"name":"处理中…","source_type":"fetch"}'
  try {
    $fetch = Invoke-RestMethod "$Base/api/libraries/$($aiLib.id)/fetch" -Method POST -Headers $auth `
      -ContentType "application/json" `
      -Body '{"prompt":"从 Element Plus 官方仓库拉取 Button 按钮组件的 Vue 源码（packages/components/button），只要 .vue 源文件","auto_name":true}'
    Write-Host "  fetch task=$($fetch.task_id)"
    $ft = $null
    for ($i = 0; $i -lt 90; $i++) {
      Start-Sleep -Seconds 2
      $ft = Invoke-RestMethod "$Base/api/tasks/$($fetch.task_id)" -Headers $auth
      Write-Host "  [$i] $($ft.progress)% $($ft.message)"
      if ($ft.status -in @("completed", "failed")) { break }
    }
    Write-Host "  result: $($ft.status) $($ft.message)"
    $aiComps = @(Invoke-RestMethod "$Base/api/libraries/$($aiLib.id)/components" -Headers $auth)
    $aiLib2 = Invoke-RestMethod "$Base/api/libraries/$($aiLib.id)" -Headers $auth
    Write-Host "  library='$($aiLib2.name)' components=$($aiComps.Count)"
    $aiComps | ForEach-Object { Write-Host "  - $($_.name)" }
  } catch {
    Write-Host "  AI fetch skipped/failed: $($_.Exception.Message)" -ForegroundColor Yellow
  }
}

$stats = Invoke-RestMethod "$Base/api/stats" -Headers $auth
$libs = @(Invoke-RestMethod "$Base/api/libraries" -Headers $auth)
Write-Host "`n=== Final ===" -ForegroundColor Green
Write-Host "libraries=$($stats.libraries) components=$($stats.components)"
$libs | ForEach-Object { Write-Host ("  {0,-24} {1,-8} count={2} status={3}" -f $_.name, $_.source_type, $_.component_count, $_.status) }
