#!/usr/bin/env python3
"""
patch_firmware_17_77.py - Générateur déterministe de firmware patché Legrand Green'Up Premium.

Ce script autonome :
1. Vérifie l'empreinte SHA-256 du firmware officiel Legrand V01.17.77 en entrée.
2. Applique de manière chirurgicale le patch de télémétrie Linky TIC (détournement de MacAdr?).
3. Génère le fichier Intel HEX patché (V01.17.78 / 17.77c).
4. Valide l'intégrité binaire et l'empreinte SHA-256 du fichier de sortie.

Zéro dépendance externe : utilise exclusivement la bibliothèque standard Python 3.
"""

import sys
import os
import hashlib
import argparse

# ==============================================================================
# SIGNATURES DE CONTRÔLE OFFICIELLES (SHA-256)
# ==============================================================================
# Firmware original V01.17.77
ORIG_SHA256_CRLF = "27e3450792989d7c06b4a60516fe0a8db53fad1281cc33c69c1108ff5006c353"
ORIG_SHA256_LF   = "0e41ee388d80e0092f56591236f80fab03f0391c7a9f46e1fa52665e81ecbc1e"
ORIG_PAYLOAD_SHA256 = "d46a6bd65804804a0783ae166b05f906705cf26e2cc84edbc96c6c760a5e04af"
ORIG_PAYLOAD_SIZE = 146028  # 0x23A6C octets

# Firmware patché généré V01.17.78 (17.77c)
OUT_SHA256_CRLF  = "1fca0124713af7cf60afa4f62313934d62fac7a854694ad93ba3d11f8b8ab139"
OUT_SHA256_LF    = "0831a304683ef5a7eadcf9a0a1cd19b4546509cda9663ce423aa29d2871704ef"
OUT_PAYLOAD_SHA256 = "261d079e8bd36fb939f4800a4f380162c0b08b1a38305a0b353028e00d45f773"
OUT_PAYLOAD_SIZE = 146570  # 0x23C8A octets

# ==============================================================================
# BYTECODE DU PATCH ASSEMBLEUR (538 octets injectés à 0x23A70)
# ==============================================================================
PATCH_HEX = (
    "cf93df939091a20b992309f473c0a091330baa2309f43dc0edeffbe38bd0eae0fce34091"
    "17025091180271d0e2e1fce340910d0250910e0264d0eae1fce340910b0250910c025dd0"
    "e2e2fce34091190250911a025cd0eae2fce3409115025091160255d0e1e3fce340911302"
    "5091140248d0eae3fce3409111025091120241d0e3e4fce340910f02509110023ad042d0"
    "34c0eeeefbe34ed040910502509106024f3f2fef5207b1f0e4e5fce34091070250910802"
    "2cd0ede5fce3409105025091060225d0e6e6fce3409103025091040218d007c0ece4fce3"
    "409107025091080216d0efe6fce34091090250910a020fd011d003c0e4eefbe31dd0df91"
    "cf910c94ca594f3f2fef520709f001d0089512d025d008958091e60a882321f0e8e7fce3"
    "09d008958091e70a882319f0e1e8fce301d008958f939f9382e08bbf67911bbe662349f0"
    "ef93ff9384ec9fe00e94efe3ff91ef91f1cf9f918f9108958f939f932ae06627772784ec"
    "9fe00e94c2ea9f918f9108955449433a415543554e005449433a484953544f5249515545"
    "005449433a5354414e44415244003b55524d53313a003b55524d53323a003b55524d5333"
    "3a003b49524d53313a003b505245463a003b53494e5354313a003b53494e5354323a003b"
    "53494e5354333a003b49494e53543a003b49494e5354313a003b49494e5354323a003b49"
    "494e5354333a003b49534f5553433a003b505445433a4843003b505445433a485000"
)
PATCH_BYTES = bytes.fromhex(PATCH_HEX)
assert len(PATCH_BYTES) == 538, f"Taille invalide du patch : {len(PATCH_BYTES)}"


# ==============================================================================
# FONCTIONS DE PARSING ET DE GÉNÉRATION INTEL HEX
# ==============================================================================
def read_intel_hex(path: str):
    """Lit un fichier Intel HEX, vérifie chaque checksum et extrait la mémoire."""
    mem = {}
    records = []
    base = 0
    with open(path, "r", encoding="latin-1") as f:
        for ln, line in enumerate(f, 1):
            line = line.strip()
            if not line:
                continue
            if line[0] != ":":
                raise ValueError(f"Ligne {ln}: enregistrement non préfixé par ':'")
            raw = bytes.fromhex(line[1:])
            n = raw[0]
            addr = (raw[1] << 8) | raw[2]
            typ = raw[3]
            data = raw[4:4 + n]
            if len(raw) != 5 + n:
                raise ValueError(f"Ligne {ln}: longueur brute incorrecte ({len(raw)})")
            if (sum(raw) & 0xFF) != 0:
                raise ValueError(f"Ligne {ln}: somme de contrôle Intel HEX invalide")

            records.append((typ, addr, data, base))
            if typ == 0:  # Données
                for i, b in enumerate(data):
                    mem[base + addr + i] = b
            elif typ == 2:  # Adresse segmentée étendue
                base = ((data[0] << 8) | data[1]) << 4
            elif typ == 4:  # Adresse linéaire étendue
                base = ((data[0] << 8) | data[1]) << 16
            elif typ == 1:  # Fin de fichier
                break

    top = max(mem.keys()) + 1
    bin_buf = bytearray(b"\xFF" * top)
    for a, b in mem.items():
        bin_buf[a] = b
    return bytearray(bin_buf), records


def format_hex_record(addr: int, typ: int, data: bytes) -> str:
    """Formate un enregistrement Intel HEX avec checksum standard."""
    raw = bytes([len(data), (addr >> 8) & 0xFF, addr & 0xFF, typ]) + bytes(data)
    cs = (-sum(raw)) & 0xFF
    return ":" + (raw + bytes([cs])).hex().upper()


# ==============================================================================
# LOGIQUE PRINCIPALE DU PATCH
# ==============================================================================
def patch_firmware(input_hex: str, output_hex: str):
    print("=" * 70)
    print("  Générateur de Firmware Patché Legrand Green'Up Premium (17.77c)")
    print("=" * 70)

    if not os.path.exists(input_hex):
        print(f"[ERREUR] Le fichier d'entrée est introuvable : {input_hex}")
        sys.exit(1)

    # 1. Vérification du hash SHA-256 du fichier d'entrée
    with open(input_hex, "rb") as f:
        raw_input = f.read()
    input_file_hash = hashlib.sha256(raw_input).hexdigest().lower()

    # 2. Lecture et décodage Intel HEX
    print(f"[*] Lecture du fichier : {input_hex}")
    bin_orig, records_orig = read_intel_hex(input_hex)
    payload_hash = hashlib.sha256(bin_orig).hexdigest().lower()

    print(f"[*] Empreinte SHA-256 fichier d'origine : {input_file_hash}")
    print(f"[*] Empreinte SHA-256 payload binaire   : {payload_hash}")
    print(f"[*] Taille du binaire original          : {len(bin_orig)} octets")

    # Validation rigoureuse
    is_valid_file = input_file_hash in (ORIG_SHA256_CRLF, ORIG_SHA256_LF)
    is_valid_payload = (payload_hash == ORIG_PAYLOAD_SHA256) and (len(bin_orig) == ORIG_PAYLOAD_SIZE)

    if not is_valid_payload and not is_valid_file:
        print("\n[ERREUR CRITIQUE] L'empreinte du fichier ne correspond PAS au firmware")
        print("                 officiel Legrand FirmwareBoardA-V01_17_77.hex !")
        print(f"  Attendu (payload) : {ORIG_PAYLOAD_SHA256}")
        print(f"  Obtenu  (payload) : {payload_hash}")
        print("Opération annulée pour éviter toute corruption de firmware.")
        sys.exit(1)

    print("[+] Signature du firmware officiel Legrand 17.77 VALIDÉE avec succès !")

    # 3. Application du patch en mémoire vive
    new_bin = bytearray(bin_orig)

    # 3.1. Trampoline MacAdr? à 0x0B630
    # jmp 0x23A70 (0D 94 38 1D) + 17 NOPs (00 00 * 17) = 38 octets
    trampoline = bytes([0x0D, 0x94, 0x38, 0x1D]) + (b"\x00\x00" * 17)
    assert len(trampoline) == 38
    print("[*] Injection du trampoline MacAdr? à 0x0B630 (38 octets)...")
    new_bin[0x0B630:0x0B630 + 38] = trampoline

    # 3.2. Incrément du numéro de version à 0x232C0 ("011777\0" -> "011778\0")
    print(f"[*] Version d'origine à 0x232C0 : {bytes(new_bin[0x232C0:0x232C7])}")
    assert new_bin[0x232C5] == ord("7"), "Octet de version inattendu à 0x232C5"
    new_bin[0x232C5] = ord("8")
    print(f"[+] Version patchée à 0x232C0   : {bytes(new_bin[0x232C0:0x232C7])} -> V01.17.78")

    # 3.3. Alignement et ajout de la routine de télémétrie TIC à 0x23A70
    assert len(new_bin) == 0x23A6C, f"Taille inattendue avant patch: 0x{len(new_bin):X}"
    new_bin.extend(b"\xFF\xFF\xFF\xFF")  # Alignement 4 octets vers 0x23A70
    assert len(new_bin) == 0x23A70
    new_bin.extend(PATCH_BYTES)
    assert len(new_bin) == OUT_PAYLOAD_SIZE, f"Taille payload résultante incorrecte: {len(new_bin)}"
    print(f"[+] Routine de télémétrie TIC injectée à 0x23A70 ({len(PATCH_BYTES)} octets)")

    # 4. Reconstruction du fichier Intel HEX
    print(f"[*] Génération du fichier Intel HEX : {output_hex}...")
    lines = []
    current_base = 0

    for typ, addr, data, base in records_orig:
        if typ == 0:  # Remplacer les octets modifiés dans les records existants
            nd = bytes(new_bin[base + addr + i] for i in range(len(data)))
            lines.append(format_hex_record(addr, 0, nd))
        elif typ == 4:
            current_base = ((data[0] << 8) | data[1]) << 16
            lines.append(format_hex_record(addr, typ, data))
        elif typ == 1:
            pass  # Ne pas écrire le EOF maintenant
        else:
            lines.append(format_hex_record(addr, typ, data))

    # Ajouter les records pour la zone étendue 0x23A70..fin
    patch_start = 0x23A70
    patch_end = len(new_bin)
    ext_base = 0x00020000

    if current_base != ext_base:
        lines.append(format_hex_record(0, 4, bytes([0x00, 0x02])))

    cur = patch_start
    while cur < patch_end:
        chunk_len = min(16, patch_end - cur)
        chunk = new_bin[cur:cur + chunk_len]
        addr = cur - ext_base
        lines.append(format_hex_record(addr, 0, chunk))
        cur += chunk_len

    # Enregistrement EOF
    lines.append(":00000001FF")

    # Écriture physique sur disque (terminateurs CRLF standard pour compatibilité maximale)
    output_content = "\r\n".join(lines) + "\r\n"
    with open(output_hex, "wb") as f:
        f.write(output_content.encode("ascii"))

    # 5. Contrôle qualité et validation du fichier généré
    print("[*] Contrôle d'intégrité du fichier généré...")
    out_bin_check, _ = read_intel_hex(output_hex)
    assert len(out_bin_check) == OUT_PAYLOAD_SIZE, "Taille du binaire généré incohérente"
    assert out_bin_check == new_bin, "Le binaire relu depuis l'Intel HEX diffère du binaire cible !"

    final_payload_hash = hashlib.sha256(out_bin_check).hexdigest().lower()
    final_file_hash = hashlib.sha256(open(output_hex, "rb").read()).hexdigest().lower()

    print(f"[*] Empreinte SHA-256 fichier de sortie : {final_file_hash}")
    print(f"[*] Empreinte SHA-256 payload binaire   : {final_payload_hash}")

    assert final_payload_hash == OUT_PAYLOAD_SHA256, "Le hash du payload de sortie ne correspond pas !"
    print("\n" + "=" * 70)
    print("  [SUCCÈS] Le firmware patché a été généré et certifié bit-for-bit !")
    print("=" * 70)
    print(f"  Fichier de sortie : {output_hex}")
    print(f"  Version logicielle: V01.17.78")
    print(f"  Taille payload    : {len(out_bin_check)} octets")
    print(f"  Empreinte SHA-256 : {final_file_hash}")
    print("=" * 70 + "\n")


def main():
    default_input = os.path.join("hex", "FirmwareBoardA-V01_17_77.hex")
    default_output = os.path.join("hex", "FirmwareBoardA-V01_17_77c.hex")

    parser = argparse.ArgumentParser(
        description="Générateur de firmware patché Legrand Green'Up Premium (17.77c)"
    )
    parser.add_argument(
        "-i", "--input", default=default_input,
        help=f"Chemin du firmware officiel Legrand 17.77 (.hex) [défaut: {default_input}]"
    )
    parser.add_argument(
        "-o", "--output", default=default_output,
        help=f"Chemin du firmware patché à générer (.hex) [défaut: {default_output}]"
    )

    args = parser.parse_args()
    patch_firmware(args.input, args.output)


if __name__ == "__main__":
    main()
