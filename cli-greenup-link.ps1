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
    Write-Host "3. Piloter la charge (Pause/Reprise via SB)"
    Write-Host "4. Piloter le verrouillage de la prise (T2C)"
    Write-Host "5. Définir la limite de courant (7A - 32A)"
    Write-Host "6. Piloter le module Bluetooth"
    Write-Host "7. Relancer l'initialisation de la borne"
    Write-Host "8. Forcer le rafraîchissement TIC"
    Write-Host "9. Redémarrer la carte de puissance (Reset)"
    Write-Host "COMMAND. Mode Expert (Commandes brutes)"
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
                $response | ConvertTo-Json -Depth 5 | Write-Host -ForegroundColor Green
            }
            "2" {
                Write-Host "`n[GET] $BASE_URL/api/telemetry..." -ForegroundColor Yellow
                $response = Invoke-RestMethod -Uri "$BASE_URL/api/telemetry" -Method Get
                $response | ConvertTo-Json -Depth 5 | Write-Host -ForegroundColor Green
            }
            "3" {
                $state = Read-Host "Démarrer (D) ou Stopper (S) la charge ? (D/S)"
                if ($state -match "^[Dd]") {
                    Write-Host "`n[POST] $BASE_URL/api/charge/start..." -ForegroundColor Yellow
                    $response = Invoke-RestMethod -Uri "$BASE_URL/api/charge/start" -Method Post
                } else {
                    Write-Host "`n[POST] $BASE_URL/api/charge/stop..." -ForegroundColor Yellow
                    $response = Invoke-RestMethod -Uri "$BASE_URL/api/charge/stop" -Method Post
                }
                $response | ConvertTo-Json | Write-Host -ForegroundColor Green
            }
            "4" {
                $state = Read-Host "Activer (A) ou Désactiver (D) la prise Type 2 ? (A/D)"
                if ($state -match "^[Aa]") {
                    Write-Host "`n[POST] $BASE_URL/api/t2/enable..." -ForegroundColor Yellow
                    $response = Invoke-RestMethod -Uri "$BASE_URL/api/t2/enable" -Method Post
                } else {
                    Write-Host "`n[POST] $BASE_URL/api/t2/disable..." -ForegroundColor Yellow
                    $response = Invoke-RestMethod -Uri "$BASE_URL/api/t2/disable" -Method Post
                }
                $response | ConvertTo-Json | Write-Host -ForegroundColor Green
            }
            "5" {
                $amps = Read-Host "Entrez la limite de courant (ex: 16)"
                Write-Host "`n[POST] $BASE_URL/api/current/$amps..." -ForegroundColor Yellow
                $response = Invoke-RestMethod -Uri "$BASE_URL/api/current/$amps" -Method Post
                $response | ConvertTo-Json | Write-Host -ForegroundColor Green
            }
            "6" {
                $state = Read-Host "Voulez-vous activer le Bluetooth ? (O/N)"
                $isEnabled = $state -match "^[OoYy]"
                $body = @{ enabled = $isEnabled } | ConvertTo-Json
                Write-Host "`n[POST] $BASE_URL/api/bluetooth..." -ForegroundColor Yellow
                $response = Invoke-RestMethod -Uri "$BASE_URL/api/bluetooth" -Method Post -Body $body -ContentType "application/json"
                $response | ConvertTo-Json | Write-Host -ForegroundColor Green
            }
            "7" {
                Write-Host "`n[POST] $BASE_URL/api/init..." -ForegroundColor Yellow
                $response = Invoke-RestMethod -Uri "$BASE_URL/api/init" -Method Post
                $response | ConvertTo-Json | Write-Host -ForegroundColor Green
            }
            "8" {
                Write-Host "`n[POST] $BASE_URL/api/tic/refresh..." -ForegroundColor Yellow
                $response = Invoke-RestMethod -Uri "$BASE_URL/api/tic/refresh" -Method Post
                $response | ConvertTo-Json | Write-Host -ForegroundColor Green
            }
            "9" {
                Write-Host "`n[POST] $BASE_URL/api/reset..." -ForegroundColor Yellow
                $response = Invoke-RestMethod -Uri "$BASE_URL/api/reset" -Method Post
                $response | ConvertTo-Json | Write-Host -ForegroundColor Green
            }
            "COMMAND" {
                Write-Host "`n--- ATTENTION: MODE COMMANDE DIRECTE ---" -ForegroundColor Red
                Write-Host "Vous écrivez directement sur le bus série de la borne." -ForegroundColor Red
                Write-Host "N'envoyez pas de commandes inconnues sous peine de risquer de corrompre ou bloquer le matériel !" -ForegroundColor Red
                Write-Host "Tapez 'exit', 'quit' ou laissez vide pour revenir au menu sécurisé." -ForegroundColor Gray
                
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
