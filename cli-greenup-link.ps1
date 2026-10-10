$PI_IP = ""
while ([string]::IsNullOrWhiteSpace($PI_IP)) {
    $PI_IP = Read-Host "Enter the Raspberry Pi IP address (e.g., 192.168.1.50)"
}
$BASE_URL = "http://${PI_IP}:8080"

function Show-Menu {
    Clear-Host
    Write-Host "=================================================" -ForegroundColor Cyan
    Write-Host "    CLI CLIENT API - GREEN'UP LINK               " -ForegroundColor White
    Write-Host "    Target: $BASE_URL                            " -ForegroundColor Gray
    Write-Host "=================================================" -ForegroundColor Cyan
    Write-Host "1. Get board info (Hardware/Firmware)"
    Write-Host "2. Get real-time telemetry"
    Write-Host "3. Control charging (Enable/Disable/Pause/Resume)"
    Write-Host "4. Set current limit (7A - 32A)"
    Write-Host "5. Control Bluetooth module"
    Write-Host "6. Restart initialization sequence"
    Write-Host "7. Force TIC refresh"
    Write-Host "8. Reboot power board (Reset)"
    Write-Host "0. Exit"
    Write-Host "=================================================" -ForegroundColor Cyan
}
while ($true) {
    Show-Menu
    $choice = Read-Host "Choose an option"

    try {
        switch ($choice) {
            "1" {
                Write-Host "`n[GET] $BASE_URL/api/info..." -ForegroundColor Yellow
                $response = Invoke-RestMethod -Uri "$BASE_URL/api/info" -Method Get
                $response | ConvertTo-Json -Depth 5 | Write-Host -ForegroundColor Green
            }
            "2" {
                Write-Host "`n[GET] $BASE_URL/api/telemetry..." -ForegroundColor Yellow
                $response = Invoke-RestMethod -Uri "$BASE_URL/api/telemetry" -Method Get
                $response | ConvertTo-Json -Depth 5 | Write-Host -ForegroundColor Green
            }
            "3" {
                $state = Read-Host "Enable (E) / Disable (D) / Pause (P) / Resume (R) the charge?"
                if ($state -match "^[Ee]") {
                    Write-Host "`n[POST] $BASE_URL/api/charge/enable..." -ForegroundColor Yellow
                    $response = Invoke-RestMethod -Uri "$BASE_URL/api/charge/enable" -Method Post
                } elseif ($state -match "^[Dd]") {
                    Write-Host "`n[POST] $BASE_URL/api/charge/disable..." -ForegroundColor Yellow
                    $response = Invoke-RestMethod -Uri "$BASE_URL/api/charge/disable" -Method Post
                } elseif ($state -match "^[Pp]") {
                    Write-Host "`n[POST] $BASE_URL/api/charge/pause..." -ForegroundColor Yellow
                    $response = Invoke-RestMethod -Uri "$BASE_URL/api/charge/pause" -Method Post
                } else {
                    Write-Host "`n[POST] $BASE_URL/api/charge/resume..." -ForegroundColor Yellow
                    $response = Invoke-RestMethod -Uri "$BASE_URL/api/charge/resume" -Method Post
                }
                $response | ConvertTo-Json | Write-Host -ForegroundColor Green
            }
            "4" {
                $amps = Read-Host "Enter current limit (e.g., 16)"
                Write-Host "`n[POST] $BASE_URL/api/current/$amps..." -ForegroundColor Yellow
                $response = Invoke-RestMethod -Uri "$BASE_URL/api/current/$amps" -Method Post
                $response | ConvertTo-Json | Write-Host -ForegroundColor Green
            }
            "5" {
                $state = Read-Host "Enable Bluetooth? (Y/N)"
                $isEnabled = $state -match "^[Yy]"
                $body = @{ enabled = $isEnabled } | ConvertTo-Json
                Write-Host "`n[POST] $BASE_URL/api/bluetooth..." -ForegroundColor Yellow
                $response = Invoke-RestMethod -Uri "$BASE_URL/api/bluetooth" -Method Post -Body $body -ContentType "application/json"
                $response | ConvertTo-Json | Write-Host -ForegroundColor Green
            }
            "6" {
                Write-Host "`n[POST] $BASE_URL/api/init..." -ForegroundColor Yellow
                $response = Invoke-RestMethod -Uri "$BASE_URL/api/init" -Method Post
                $response | ConvertTo-Json | Write-Host -ForegroundColor Green
            }
            "7" {
                Write-Host "`n[POST] $BASE_URL/api/tic/refresh..." -ForegroundColor Yellow
                $response = Invoke-RestMethod -Uri "$BASE_URL/api/tic/refresh" -Method Post
                $response | ConvertTo-Json | Write-Host -ForegroundColor Green
            }
            "8" {
                Write-Host "`n[POST] $BASE_URL/api/reset..." -ForegroundColor Yellow
                $response = Invoke-RestMethod -Uri "$BASE_URL/api/reset" -Method Post
                $response | ConvertTo-Json | Write-Host -ForegroundColor Green
            }
            "COMMAND" {
                Write-Host "`n--- WARNING: DIRECT COMMAND MODE ---" -ForegroundColor Red
                Write-Host "You are writing directly to the station's serial bus." -ForegroundColor Red
                Write-Host "Do not send unknown commands to avoid corrupting or locking the hardware!" -ForegroundColor Red
                Write-Host "Type 'exit', 'quit' or leave empty to return to the safe menu." -ForegroundColor Gray
                
                while ($true) {
                    $cmd = Read-Host "`nPS> Raw command"
                    
                    if ([string]::IsNullOrWhiteSpace($cmd) -or $cmd.ToLower() -match "^(exit|quit)$") {
                        Write-Host "Returning to main menu..." -ForegroundColor Gray
                        break
                    }

                    Write-Host "[POST] $BASE_URL/api/command -> '$cmd'" -ForegroundColor Yellow
                    try {
                        $response = Invoke-RestMethod -Uri "$BASE_URL/api/command" -Method Post -Body $cmd -ContentType "text/plain"
                        $response | ConvertTo-Json -Depth 2 | Write-Host -ForegroundColor Green
                    } catch {
                        Write-Host "❌ Request error: $($_.Exception.Message)" -ForegroundColor Red
                    }
                }
            }
            "0" {
                Write-Host "`nGoodbye!" -ForegroundColor Cyan
                exit
            }
            default {
                Write-Host "`nInvalid option." -ForegroundColor Red
            }
        }
    } catch {
        Write-Host "`n❌ Communication error: $($_.Exception.Message)" -ForegroundColor Red
        Write-Host "Is the Raspberry Pi server running?" -ForegroundColor Gray
    }

    Write-Host "`nPress Enter to continue..." -ForegroundColor DarkGray
    $null = Read-Host
}
