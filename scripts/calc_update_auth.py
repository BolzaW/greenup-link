#!/usr/bin/env python3
"""
calc_update_auth.py - Calculateur de code de déverrouillage bootloader (UpdateAuth)
pour bornes de recharge Legrand Green'Up Premium.

Usage :
  1. Avec arguments positionnels :
     python3 calc_update_auth.py 42W22 V01.17.78 015184 V02.01.01 058001

  2. Avec options nommées :
     python3 calc_update_auth.py --date 42W22 --sw V01.17.78 --sn 015184 --hw V02.01.01 --ref 058001

  3. Automatique via l'API REST greenup-link (port 8080) :
     python3 calc_update_auth.py --api http://localhost:8080
     # Pour armer directement le bootloader :
     python3 calc_update_auth.py --api http://localhost:8080 --arm

  4. Mode interactif (sans arguments) :
     python3 calc_update_auth.py
"""

import sys
import argparse
import urllib.request
import urllib.parse
import json
import re


def crc16_modbus(data: bytes) -> int:
    """Calcule le CRC16-Modbus standard (polynôme inversé 0xA001, initial 0xFFFF)."""
    crc = 0xFFFF
    for b in data:
        crc ^= b
        for _ in range(8):
            if crc & 1:
                crc = (crc >> 1) ^ 0xA001
            else:
                crc >>= 1
    return crc


def normalize_version(v: str) -> str:
    """Normalise une chaîne de version en format Vxx.yy.zz (9 caractères)."""
    v = v.strip().replace(":", "").replace(" ", "")
    # Supprimer les préfixes comme 'SoftwareVersion:' ou 'HardwareVersion:'
    if "=" in v:
        v = v.split("=")[-1]
    if ":" in v:
        v = v.split(":")[-1]

    if not v.startswith("V") and not v.startswith("v"):
        if "." in v:
            v = "V" + v
        elif len(v) == 6:
            v = f"V{v[0:2]}.{v[2:4]}.{v[4:6]}"
        else:
            v = "V" + v
    else:
        v = "V" + v[1:]

    # Si format V011778 sans points
    if len(v) == 7 and "." not in v:
        v = f"V{v[1:3]}.{v[3:5]}.{v[5:7]}"

    if len(v) != 9 or v[3] != "." or v[6] != ".":
        raise ValueError(
            f"Format de version invalide '{v}' (attendu: 9 caractères type 'V01.17.78')"
        )
    return v


def compute_update_auth(datedefab: str, sw: str, sn: str, hw: str, ref: str):
    """
    Calcule la clé entrelacée de 28 caractères et son CRC16 Modbus.
    Retourne un tuple: (key_28, crc_code, command_str).
    """
    sw_norm = normalize_version(sw)
    hw_norm = normalize_version(hw)
    date_clean = datedefab.strip().split(":")[-1].strip()
    sn_clean = sn.strip().split(":")[-1].strip()
    ref_clean = ref.strip().split(":")[-1].strip()

    if len(date_clean) != 5:
        raise ValueError(f"Date de production invalide '{date_clean}' (attendu: 5 car., ex: '42W22')")
    if len(sn_clean) != 6:
        raise ValueError(f"Numéro de série invalide '{sn_clean}' (attendu: 6 car., ex: '015184')")
    if len(ref_clean) != 6:
        raise ValueError(f"Référence invalide '{ref_clean}' (attendu: 6 car., ex: '058001')")

    # Formule officielle Legrand d'entrelacement (28 caractères)
    chars = [
        date_clean[0], sw_norm[8], sn_clean[2], hw_norm[8], ref_clean[1],
        date_clean[4], sw_norm[1], sn_clean[1], hw_norm[7], ref_clean[3],
        date_clean[1], sw_norm[4], sn_clean[5], hw_norm[1], ref_clean[4],
        date_clean[3], sw_norm[2], sn_clean[0], hw_norm[2], ref_clean[5],
        sw_norm[5], sn_clean[4], hw_norm[4], ref_clean[2],
        sw_norm[7], sn_clean[3], hw_norm[5], ref_clean[0]
    ]

    key_28 = "".join(chars)
    crc = crc16_modbus(key_28.encode("ascii"))
    command = f"UpdateAuth:{crc}"
    return key_28, crc, command


def query_api_cmd(base_url: str, cmd: str) -> str:
    """Envoie une commande série à l'API greenup-link et renvoie la réponse."""
    url = f"{base_url.rstrip('/')}/api/command"
    req = urllib.request.Request(
        url,
        data=cmd.encode("utf-8"),
        headers={"Content-Type": "text/plain"},
        method="POST"
    )
    with urllib.request.urlopen(req, timeout=5.0) as resp:
        body = resp.read().decode("utf-8")
        try:
            data = json.loads(body)
            # Supporte { "response": "..." } ou retour brut
            if isinstance(data, dict) and "response" in data:
                return data["response"]
        except json.JSONDecodeError:
            pass
        return body


def auto_from_api(base_url: str, arm: bool):
    """Interroge automatiquement la borne via l'API REST."""
    print(f"[*] Connexion à l'API greenup-link sur {base_url}...")
    try:
        date = query_api_cmd(base_url, "WeekYearProduction?")
        sw = query_api_cmd(base_url, "SoftwareVersion?")
        sn = query_api_cmd(base_url, "SerialNumber?")
        hw = query_api_cmd(base_url, "HardwareVersion?")
        ref = query_api_cmd(base_url, "Reference?")
    except Exception as e:
        print(f"[!] Échec de communication avec l'API : {e}")
        sys.exit(1)

    # Nettoyage des préfixes éventuels
    clean = lambda s: re.sub(r"^[^:]+:", "", s).strip() if ":" in s else s.strip()
    date, sw, sn, hw, ref = clean(date), clean(sw), clean(sn), clean(hw), clean(ref)

    print("\n--- Caractéristiques détectées ---")
    print(f"  • Date de fabrication (WeekYearProduction) : {date}")
    print(f"  • Version logicielle (SoftwareVersion)      : {sw}")
    print(f"  • Numéro de série    (SerialNumber)        : {sn}")
    print(f"  • Version matérielle (HardwareVersion)      : {hw}")
    print(f"  • Référence borne    (Reference)            : {ref}")

    key_28, crc, cmd = compute_update_auth(date, sw, sn, hw, ref)
    display_results(key_28, crc, cmd, base_url)

    if arm:
        print(f"\n[*] Armement du bootloader avec '{cmd}'...")
        try:
            resp = query_api_cmd(base_url, cmd)
            print(f"[+] Réponse de la carte : {resp}")
            if "UpdateCodeOK" in resp:
                print(">>> SUCCÈS : Bootloader armé ! Vous pouvez maintenant couper greenup-link et lancer avrdude.")
            else:
                print(f"[!] Attention : la carte n'a pas renvoyé 'UpdateCodeOK' (reçu: {resp})")
        except Exception as e:
            print(f"[!] Erreur lors de l'envoi de la commande : {e}")


def display_results(key_28: str, crc: int, cmd: str, base_url: str = "http://localhost:8080"):
    print("\n" + "=" * 65)
    print(f"  Clé entrelacée Legrand (28 car.) : {key_28}")
    print(f"  Code CRC16-Modbus calculé        : {crc}")
    print(f"  Commande série exacte            : {cmd}")
    print("=" * 65)
    print("\nCommande curl pour armer le bootloader :")
    print(f"  curl -X POST {base_url}/api/command -d \"{cmd}\"\n")


def main():
    parser = argparse.ArgumentParser(
        description="Calculateur universel de code UpdateAuth Legrand Green'Up Premium"
    )
    parser.add_argument("params", nargs="*", help="Arguments positionnels: DATE SW SN HW REF")
    parser.add_argument("--date", help="Semaine et année de fabrication (ex: 42W22)")
    parser.add_argument("--sw", help="Version logicielle active avant flash (ex: V01.17.78)")
    parser.add_argument("--sn", help="Numéro de série de la borne (ex: 015184)")
    parser.add_argument("--hw", help="Version matérielle de la borne (ex: V02.01.01)")
    parser.add_argument("--ref", help="Référence commerciale de la borne (ex: 058001)")
    parser.add_argument("--api", nargs="?", const="http://localhost:8080", help="URL API REST greenup-link")
    parser.add_argument("--arm", action="store_true", help="Envoie directement la commande UpdateAuth à l'API")

    args = parser.parse_args()

    # 1. Mode API
    if args.api:
        auto_from_api(args.api, args.arm)
        return

    # 2. Mode Arguments nommés
    if args.date and args.sw and args.sn and args.hw and args.ref:
        key_28, crc, cmd = compute_update_auth(args.date, args.sw, args.sn, args.hw, args.ref)
        display_results(key_28, crc, cmd)
        return

    # 3. Mode Arguments positionnels (5 attendus)
    if len(args.params) == 5:
        date, sw, sn, hw, ref = args.params
        key_28, crc, cmd = compute_update_auth(date, sw, sn, hw, ref)
        display_results(key_28, crc, cmd)
        return

    # 4. Mode interactif si aucun argument fourni
    if len(args.params) == 0 and not (args.date or args.sw or args.sn or args.hw or args.ref):
        print("=== Calculateur de Code UpdateAuth Legrand ===")
        print("Saisissez les 5 valeurs obtenues de la borne (ou Ctrl+C pour quitter) :\n")
        try:
            date = input("  1. Semaine/Année de fabrication [WeekYearProduction?] (ex: 42W22) : ").strip()
            sw   = input("  2. Version logicielle active [SoftwareVersion?] (ex: V01.17.78)   : ").strip()
            sn   = input("  3. Numéro de série [SerialNumber?] (ex: 015184)                   : ").strip()
            hw   = input("  4. Version matérielle [HardwareVersion?] (ex: V02.01.01)          : ").strip()
            ref  = input("  5. Référence commerciale [Reference?] (ex: 058001)                : ").strip()

            key_28, crc, cmd = compute_update_auth(date, sw, sn, hw, ref)
            display_results(key_28, crc, cmd)
        except (KeyboardInterrupt, EOFError):
            print("\nAnnulé.")
            sys.exit(0)
        return

    parser.print_help()
    sys.exit(1)


if __name__ == "__main__":
    main()
