$PI_IP = ""
while ([string]::IsNullOrWhiteSpace($PI_IP)) {
    $PI_IP = Read-Host "Entrez l'adresse IP du Raspberry Pi (ex: 192.168.1.50)"
}
$BASE_URL = "http://${PI_IP}:8080"

function Show-Menu {
    Clear-Host
    Write-Host "=================================================" -ForegroundColor Cyan
    Write-Host "    CLI CLIENT API - GREEN'UP LINK               " -ForegroundColor White
    Write-Host "    Cible : $BASE_URL                            " -ForegroundColor Gray
    Write-Host "=================================================" -ForegroundColor Cyan
    Write-Host "1. Obtenir les informations de la borne (Info)"
    Write-Host "2. Obtenir la télémesure en temps réel"
    Write-Host "3. Démarrer la charge (T2COK)"
    Write-Host "4. Stopper la charge (T2CNOK)"
    Write-Host "5. Définir la limite de courant (10A - 32A)"
    Write-Host "6. Envoyer une commande brute (ex: State?)"
    Write-Host "0. Quitter"
    Write-Host "=================================================" -ForegroundColor Cyan
}

while ($true) {
    Show-Menu
    $choice = Read-Host "Choisissez une option"

    try {
        switch ($choice) {
            "1" {
                Write-Host "`n[GET] $BASE_URL/api/info..." -ForegroundColor Yellow
                $response = Invoke-RestMethod -Uri "$BASE_URL/api/info" -Method Get
                $response | ConvertTo-Json | Write-Host -ForegroundColor Green
            }
            "2" {
                Write-Host "`n[GET] $BASE_URL/api/telemetry..." -ForegroundColor Yellow
                $response = Invoke-RestMethod -Uri "$BASE_URL/api/telemetry" -Method Get
                $response | ConvertTo-Json | Write-Host -ForegroundColor Green
            }
            "3" {
                Write-Host "`n[POST] $BASE_URL/api/charge/start..." -ForegroundColor Yellow
                $response = Invoke-RestMethod -Uri "$BASE_URL/api/charge/start" -Method Post
                $response | ConvertTo-Json | Write-Host -ForegroundColor Green
            }
            "4" {
                Write-Host "`n[POST] $BASE_URL/api/charge/stop..." -ForegroundColor Yellow
                $response = Invoke-RestMethod -Uri "$BASE_URL/api/charge/stop" -Method Post
                $response | ConvertTo-Json | Write-Host -ForegroundColor Green
            }
            "5" {
                $amps = Read-Host "Entrez le courant en Ampères (ex: 16)"
                Write-Host "`n[POST] $BASE_URL/api/current/$amps..." -ForegroundColor Yellow
                $response = Invoke-RestMethod -Uri "$BASE_URL/api/current/$amps" -Method Post
                $response | ConvertTo-Json | Write-Host -ForegroundColor Green
            }
            "6" {
                Write-Host "`n--- MODE COMMANDE INTERACTIF ---" -ForegroundColor Cyan
                Write-Host "Tapez 'exit', 'quit' ou laissez vide pour revenir au menu." -ForegroundColor Gray
                
                while ($true) {
                    $cmd = Read-Host "`nPS> Commande brute"
                    
                    if ([string]::IsNullOrWhiteSpace($cmd) -or $cmd.ToLower() -match "^(exit|quit)$") {
                        Write-Host "Retour au menu principal..." -ForegroundColor Gray
                        break
                    }

                    Write-Host "[POST] $BASE_URL/api/command -> '$cmd'" -ForegroundColor Yellow
                    try {
                        $response = Invoke-RestMethod -Uri "$BASE_URL/api/command" -Method Post -Body $cmd -ContentType "text/plain"
                        $response | ConvertTo-Json -Depth 2 | Write-Host -ForegroundColor Green
                    } catch {
                        Write-Host "❌ Erreur de requête : $($_.Exception.Message)" -ForegroundColor Red
                    }
                }
            }
            "0" {
                Write-Host "`nAu revoir !" -ForegroundColor Cyan
                exit
            }
            default {
                Write-Host "`nOption invalide." -ForegroundColor Red
            }
        }
    } catch {
        Write-Host "`n❌ Erreur de communication : $($_.Exception.Message)" -ForegroundColor Red
        Write-Host "Le serveur sur le Raspberry Pi est-il bien lancé ?" -ForegroundColor Gray
    }

    Write-Host "`nAppuyez sur Entrée pour continuer..." -ForegroundColor DarkGray
    $null = Read-Host
}
