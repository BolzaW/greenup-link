# Green'Up Link - Interactive Command CLI
# Sends raw serial commands directly to the ATmega power board via /api/command

$PI_IP = ""
while ([string]::IsNullOrWhiteSpace($PI_IP)) {
    $PI_IP = Read-Host "Enter the Raspberry Pi IP address (e.g., 192.168.1.50)"
}
$BASE_URL = "http://${PI_IP}:8080"

$API_TOKEN = Read-Host "Enter API token (press Enter if authentication is disabled)"
$AUTH_HEADERS = @{}
if (-not [string]::IsNullOrWhiteSpace($API_TOKEN)) {
    $AUTH_HEADERS["Authorization"] = "Bearer $($API_TOKEN.Trim())"
}

Clear-Host
Write-Host "=================================================" -ForegroundColor Cyan
Write-Host "    GREEN'UP LINK - RAW SERIAL COMMAND CLI       " -ForegroundColor White
Write-Host "    Target: $BASE_URL/api/command                " -ForegroundColor Gray
Write-Host "=================================================" -ForegroundColor Cyan
Write-Host "Type any raw command to send to the station (e.g., State?, Reset, TICTM:1, CCEl:16)." -ForegroundColor Yellow
Write-Host "Type 'exit' or 'quit' to quit.`n" -ForegroundColor DarkGray

while ($true) {
    $cmd = Read-Host "GreenUp> "
    
    if ([string]::IsNullOrWhiteSpace($cmd)) {
        continue
    }
    if ($cmd.ToLower() -match "^(exit|quit)$") {
        Write-Host "Goodbye!" -ForegroundColor Cyan
        break
    }

    try {
        $response = Invoke-RestMethod -Uri "$BASE_URL/api/command" -Method Post -Headers $AUTH_HEADERS -Body $cmd.Trim() -ContentType "text/plain"
        if ($response.status -eq "success") {
            Write-Host "✓ [SUCCESS] $($response.message)" -ForegroundColor Green
        } else {
            $response | ConvertTo-Json -Depth 2 | Write-Host -ForegroundColor Yellow
        }
    } catch {
        Write-Host "❌ Error: $($_.Exception.Message)" -ForegroundColor Red
        if ($_.Exception.Response) {
            $reader = New-Object System.IO.StreamReader($_.Exception.Response.GetResponseStream())
            $body = $reader.ReadToEnd()
            Write-Host "   Server response: $body" -ForegroundColor DarkYellow
        }
    }
}
