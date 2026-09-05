param(
    [string]$BaseUrl = "http://127.0.0.1:8080",
    [string]$ApiKey = $env:COMPIRA_API_KEY,
    [string]$Query = "Button"
)

$ErrorActionPreference = "Stop"

if (-not $ApiKey) {
    Write-Error "Set COMPIRA_API_KEY env var or pass -ApiKey"
}

$headers = @{
    "X-API-Key"    = $ApiKey
    "Content-Type" = "application/json"
    "Accept"       = "application/json, text/event-stream"
}

function Invoke-McpRequest {
    param([hashtable]$Body, [string]$SessionId = $null)
    $h = $headers.Clone()
    if ($SessionId) { $h["Mcp-Session-Id"] = $SessionId }
    $json = $Body | ConvertTo-Json -Depth 10 -Compress
    $resp = Invoke-WebRequest -Uri "$BaseUrl/mcp" -Method POST -Headers $h -Body $json -UseBasicParsing
    $session = $resp.Headers["Mcp-Session-Id"]
    $lines = $resp.Content -split "`n"
    $dataLines = $lines | Where-Object { $_ -like "data: *" -and $_ -notmatch "^data:\s*$" }
    $results = @()
    foreach ($line in $dataLines) {
        $jsonStr = $line -replace "^data:\s*", ""
        if ($jsonStr.Trim()) {
            $results += $jsonStr | ConvertFrom-Json
        }
    }
    return @{ SessionId = $session; Results = $results; Raw = $resp.Content }
}

Write-Host "=== CompiraMCP Streamable HTTP 联调 ===" -ForegroundColor Cyan
Write-Host "Endpoint: $BaseUrl/mcp`n"

# 1. initialize
Write-Host "[1] initialize" -ForegroundColor Yellow
$init = Invoke-McpRequest -Body @{
    jsonrpc = "2.0"
    method  = "initialize"
    params  = @{
        protocolVersion = "2024-11-05"
        capabilities    = @{}
        clientInfo      = @{ name = "mcp-test"; version = "1.0" }
    }
    id = 1
}
$sessionId = $init.SessionId
$init.Result | ForEach-Object { Write-Host ($_.result | ConvertTo-Json -Depth 4) }

# 2. initialized notification
Write-Host "`n[2] notifications/initialized" -ForegroundColor Yellow
Invoke-McpRequest -Body @{
    jsonrpc = "2.0"
    method  = "notifications/initialized"
} -SessionId $sessionId | Out-Null

# 3. tools/list
Write-Host "`n[3] tools/list" -ForegroundColor Yellow
$list = Invoke-McpRequest -Body @{
    jsonrpc = "2.0"
    method  = "tools/list"
    params  = @{}
    id      = 2
} -SessionId $sessionId
$tools = ($list.Results | Where-Object { $_.result.tools }).result.tools
$tools | ForEach-Object { Write-Host "  - $($_.name)" }

# 4. tools/call - list_libraries
Write-Host "`n[4] tools/call list_libraries" -ForegroundColor Yellow
$call1 = Invoke-McpRequest -Body @{
    jsonrpc = "2.0"
    method  = "tools/call"
    params  = @{
        name      = "list_libraries"
        arguments = @{}
    }
    id = 3
} -SessionId $sessionId
($call1.Results | Where-Object { $_.result }).result.content[0].text

# 5. tools/call - search_components
Write-Host "`n[5] tools/call search_components (query=$Query)" -ForegroundColor Yellow
$call2 = Invoke-McpRequest -Body @{
    jsonrpc = "2.0"
    method  = "tools/call"
    params  = @{
        name      = "search_components"
        arguments = @{ query = $Query; limit = 5 }
    }
    id = 4
} -SessionId $sessionId
$searchText = ($call2.Results | Where-Object { $_.result }).result.content[0].text
Write-Host $searchText

Write-Host "`n=== 联调完成 ===" -ForegroundColor Green
