#!/bin/bash
echo "=== Redémarrage des services via les scripts natifs Legrand ==="

# On utilise les scripts init.d d'origine de la borne
sudo systemctl start tomcat9
sudo /etc/init.d/CommunicationArduinoRasp start

echo "✅ Les services natifs Legrand ont été relancés."
