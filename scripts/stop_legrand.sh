#!/bin/bash
echo "=== Arrêt des services Legrand ==="

# 1. On arrête proprement Tomcat9 via systemd
echo "[1/3] Arrêt de Tomcat9..."
sudo systemctl stop tomcat9

# 2. On arrête le démon Java Legrand. 
# Comme on ne connaît pas forcément le nom exact du service systemd, on tue tous les processus Java restants.
echo "[2/3] Arrêt forcé des autres processus Java (Daemon)..."
sudo pkill -9 -f java

# 3. Vérification
echo "[3/3] Vérification..."
sleep 1
if pgrep -f java > /dev/null
then
    echo "❌ Attention : des processus Java tournent encore !"
else
    echo "✅ Succès : Tous les services Legrand sont arrêtés. Le port série est libre."
fi
