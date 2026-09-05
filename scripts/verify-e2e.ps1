# CompiraMCP end-to-end verification
param(
    [string]$BaseUrl = "http://127.0.0.1:8080",
    [string]$SampleVue = "d:\code\前端MCP\verify-data\sample\DemoButton.vue"
)

$ErrorActionPreference = "Continue"
$results = @()

function Add-Result($name, $ok, $detail) {
    $script:results += [pscustomobject]@{ Test = $name; OK = $ok; Detail = $detail }
    $color = if ($ok) { "Green" } else { "Red" }
    Write-Host ("[{0}] {1} - {2}" -f $(if ($ok) { "PASS" } else { "FAIL" }), $name, $detail) -ForegroundColor $color
}

function Invoke-Api {
    param(
        [string]$Method = "GET",
        [string]$Path,
        [hashtable]$Headers = @{},
        [object]$Body = $null,
        [int[]]$ExpectStatus = @(200)
    )
    $h = @{ "Content-Type" = "application/json" } + $Headers
    $uri = "$BaseUrl/api$Path"
    try {
        if ($Body -ne $null) {
            $json = $Body | ConvertTo-Json -Depth 10 -Compress
            $resp = Invoke-WebRequest -Uri $uri -Method $Method -Headers $h -Body $json -UseBasicParsing
        } else {
            $resp = Invoke-WebRequest -Uri $uri -Method $Method -Headers $h -UseBasicParsing
        }
        $ok = $ExpectStatus -contains $resp.StatusCode
        $content = $resp.Content
        if ($content) {
            try { return @{ Ok = $ok; Status = $resp.StatusCode; Data = ($content | ConvertFrom-Json) } }
            catch { return @{ Ok = $ok; Status = $resp.StatusCode; Data = $content } }
        }
        return @{ Ok = $ok; Status = $resp.StatusCode; Data = $null }
    } catch {
        $status = $_.Exception.Response.StatusCode.value__
        $body = ""
        try { $reader = New-Object System.IO.StreamReader($_.Exception.Response.GetResponseStream()); $body = $reader.ReadToEnd() } catch {}
        $ok = $ExpectStatus -contains $status
        return @{ Ok = $ok; Status = $status; Data = $body; Error = $_.Exception.Message }
    }
}

Write-Host "`n=== CompiraMCP E2E Verification ===" -ForegroundColor Cyan
Write-Host "Base: $BaseUrl`n"

# 1 Health
$r = Invoke-Api -Path "/health"
Add-Result "Health" $r.Ok "status=$($r.Status)"

# 2 Login
$r = Invoke-Api -Method POST -Path "/auth/login" -Body @{ username = "admin"; password = "admin123" }
$token = $r.Data.token
Add-Result "Login admin" ($r.Ok -and $token) "role=$($r.Data.user.role)"
$auth = @{ Authorization = "Bearer $token" }

# 3 Me
$r = Invoke-Api -Path "/auth/me" -Headers $auth
Add-Result "Auth /me" ($r.Ok -and $r.Data.user.username -eq "admin") $r.Data.user.username

# 4 Stats
$r = Invoke-Api -Path "/stats" -Headers $auth
Add-Result "Stats" $r.Ok "libs=$($r.Data.libraries) components=$($r.Data.components)"

# 5 AI status
$r = Invoke-Api -Path "/ai/status" -Headers $auth
$aiEnabled = $r.Data.enabled
Add-Result "AI status" $r.Ok "enabled=$aiEnabled model=$($r.Data.model)"

# 6 Create git library
$r = Invoke-Api -Method POST -Path "/libraries" -Headers $auth -Body @{
    name = "verify-git"; repo_url = "https://github.com/vuejs/core.git"; branch = "main"; source_type = "git"
} -ExpectStatus @(200, 201)
$gitLibId = $r.Data.id
Add-Result "Create Git library" ($r.Ok -and $gitLibId) $gitLibId

# 7 Create upload library
$r = Invoke-Api -Method POST -Path "/libraries" -Headers $auth -Body @{
    name = "verify-upload"; source_type = "upload"
} -ExpectStatus @(200, 201)
$uploadLibId = $r.Data.id
Add-Result "Create upload library" ($r.Ok -and $uploadLibId) $uploadLibId

# 8 Upload component file
$uploadOk = $false
$uploadTaskId = $null
if ($uploadLibId -and (Test-Path $SampleVue)) {
    try {
        $boundary = [System.Guid]::NewGuid().ToString()
        $fileBytes = [System.IO.File]::ReadAllBytes($SampleVue)
        $fileName = "DemoButton.vue"
        $enc = [System.Text.Encoding]::UTF8
        $LF = "`r`n"
        $bodyParts = @()
        $bodyParts += "--$boundary"
        $bodyParts += "Content-Disposition: form-data; name=`"use_ai`""
        $bodyParts += ""
        $bodyParts += "false"
        $bodyParts += "--$boundary"
        $bodyParts += "Content-Disposition: form-data; name=`"files`"; filename=`"$fileName`""
        $bodyParts += "Content-Type: application/octet-stream"
        $bodyParts += ""
        $header = ($bodyParts -join $LF) + $LF
        $footer = "$LF--$boundary--$LF"
        $headerBytes = $enc.GetBytes($header)
        $footerBytes = $enc.GetBytes($footer)
        $ms = New-Object System.IO.MemoryStream
        $ms.Write($headerBytes, 0, $headerBytes.Length)
        $ms.Write($fileBytes, 0, $fileBytes.Length)
        $ms.Write($footerBytes, 0, $footerBytes.Length)
        $resp = Invoke-WebRequest -Uri "$BaseUrl/api/libraries/$uploadLibId/upload" -Method POST `
            -Headers @{ Authorization = "Bearer $token"; "Content-Type" = "multipart/form-data; boundary=$boundary" } `
            -Body $ms.ToArray() -UseBasicParsing
        $uploadData = $resp.Content | ConvertFrom-Json
        $uploadTaskId = $uploadData.task_id
        $uploadOk = $resp.StatusCode -eq 202 -and $uploadTaskId
    } catch {
        $uploadOk = $false
    }
}
Add-Result "Upload component" $uploadOk "task=$uploadTaskId"

# 9 Create AI fetch library + fetch (expect fail if no LLM)
$r = Invoke-Api -Method POST -Path "/libraries" -Headers $auth -Body @{
    name = "verify-ai"; source_type = "fetch"
} -ExpectStatus @(200, 201)
$aiLibId = $r.Data.id
Add-Result "Create AI library" ($r.Ok -and $aiLibId) $aiLibId

$r = Invoke-Api -Method POST -Path "/libraries/$aiLibId/fetch" -Headers $auth -Body @{
    prompt = "拉取 Element Plus Button 组件 Vue 源码"
} -ExpectStatus @(202, 400)
$aiFetchStarted = $r.Status -eq 202
$aiFetchExpectedFail = (-not $aiEnabled) -and ($r.Status -eq 400)
Add-Result "AI fetch (prompt only)" ($aiFetchStarted -or $aiFetchExpectedFail) "status=$($r.Status) enabled=$aiEnabled"

# 10 Poll upload task
function Wait-Task($taskId, [int]$sec = 30) {
    if (-not $taskId) { return $null }
    for ($i = 0; $i -lt $sec; $i++) {
        Start-Sleep -Seconds 1
        $t = Invoke-Api -Path "/tasks/$taskId" -Headers $auth
        if ($t.Data.status -in @("completed", "failed")) { return $t.Data }
    }
    return $null
}

$uploadTask = Wait-Task $uploadTaskId 20
Add-Result "Upload ingest task" ($uploadTask -and $uploadTask.status -eq "completed") "$($uploadTask.status) $($uploadTask.message)"

# 11 List upload components
$r = Invoke-Api -Path "/libraries/$uploadLibId/components" -Headers $auth
$comp = $r.Data | Select-Object -First 1
Add-Result "List components" ($r.Ok -and $comp) "count=$($r.Data.Count) name=$($comp.name)"
$compId = $comp.id

# 12 Component detail APIs
if ($compId) {
    $r1 = Invoke-Api -Path "/components/$compId" -Headers $auth
    $r2 = Invoke-Api -Path "/components/$compId/source" -Headers $auth
    $r3 = Invoke-Api -Path "/search/components?q=Demo" -Headers $auth
    Add-Result "Component detail+source+search" ($r1.Ok -and $r2.Ok -and $r3.Ok) "search=$($r3.Data.Count)"
}

# 13 MCP REST proxy
$r = Invoke-Api -Path "/mcp/tools" -Headers $auth
$toolCount = @($r.Data).Count
$r2 = Invoke-Api -Method POST -Path "/mcp/call" -Headers $auth -Body @{ tool = "list_libraries"; arguments = @{} }
Add-Result "MCP tools/call" ($r.Ok -and $r2.Ok -and $toolCount -ge 1) "tools=$toolCount ok=$($r2.Data.ok)"

# 14 Users
$r = Invoke-Api -Path "/users" -Headers $auth
Add-Result "List users (admin)" ($r.Ok -and $r.Data.Count -ge 1) "count=$($r.Data.Count)"

$newUser = "verifyuser$([random]::new().Next(1000,9999))"
$r = Invoke-Api -Method POST -Path "/users" -Headers $auth -Body @{
    username = $newUser; password = "verify123"; role = "user"
} -ExpectStatus @(200, 201)
Add-Result "Create user" $r.Ok $newUser

$r = Invoke-Api -Method POST -Path "/auth/login" -Body @{ username = $newUser; password = "verify123" }
$userToken = $r.Data.token
Add-Result "Login new user" ($r.Ok -and $userToken) $newUser

$r = Invoke-Api -Path "/users" -Headers @{ Authorization = "Bearer $userToken" } -ExpectStatus @(200, 403)
Add-Result "User forbidden on /users" ($r.Status -eq 403) "status=$($r.Status)"

# 15 API keys
$r = Invoke-Api -Path "/api-keys" -Headers $auth
Add-Result "List API keys" $r.Ok "count=$($r.Data.Count)"

$r = Invoke-Api -Method POST -Path "/api-keys" -Headers $auth -Body @{ name = "verify-key" } -ExpectStatus @(200, 201)
$mcpKey = $r.Data.key.key
Add-Result "Create API key" ($r.Ok -and $mcpKey) $r.Data.key.key_prefix

# 16 MCP with API key
if ($mcpKey) {
    $r = Invoke-Api -Path "/stats" -Headers @{ "X-API-Key" = $mcpKey }
    Add-Result "API key auth" $r.Ok "stats ok"
}

# 17 Logs
$r = Invoke-Api -Path "/logs?limit=10" -Headers $auth
Add-Result "Logs" ($r.Ok -and $r.Data.Count -ge 0) "count=$($r.Data.Count)"

# Summary
Write-Host "`n=== Summary ===" -ForegroundColor Cyan
$passed = ($results | Where-Object OK).Count
$failed = ($results | Where-Object { -not $_.OK }).Count
Write-Host "PASS: $passed  FAIL: $failed  TOTAL: $($results.Count)"
if ($failed -gt 0) {
    Write-Host "`nFailed:" -ForegroundColor Yellow
    $results | Where-Object { -not $_.OK } | Format-Table -AutoSize
    exit 1
}
exit 0
