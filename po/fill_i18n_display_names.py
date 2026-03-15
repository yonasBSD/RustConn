#!/usr/bin/env python3
"""Fill translations for protocol display_name() strings in all .po files."""

import re
import os
import sys

TRANSLATIONS = {
    "uk": {
        "External RDP client": "Зовнішній RDP-клієнт",
        "External VNC client": "Зовнішній VNC-клієнт",
        "Quality (RemoteFX)": "Якість (RemoteFX)",
        "Balanced (Adaptive)": "Збалансований (адаптивний)",
        "Speed (Legacy)": "Швидкість (застарілий)",
        "Balanced": "Збалансований",
        "Speed": "Швидкість",
        "Auto (system)": "Авто (системний)",
        "Odd": "Непарна",
        "Even": "Парна",
        "Hardware (RTS/CTS)": "Апаратне (RTS/CTS)",
        "Software (XON/XOFF)": "Програмне (XON/XOFF)",
        "Automatic": "Автоматично",
        "Backspace (^H)": "Backspace (^H)",
        "Delete (^?)": "Delete (^?)",
    },
    "de": {
        "External RDP client": "Externer RDP-Client",
        "External VNC client": "Externer VNC-Client",
        "Quality (RemoteFX)": "Qualität (RemoteFX)",
        "Balanced (Adaptive)": "Ausgewogen (Adaptiv)",
        "Speed (Legacy)": "Geschwindigkeit (Legacy)",
        "Balanced": "Ausgewogen",
        "Speed": "Geschwindigkeit",
        "Auto (system)": "Auto (System)",
        "Odd": "Ungerade",
        "Even": "Gerade",
        "Hardware (RTS/CTS)": "Hardware (RTS/CTS)",
        "Software (XON/XOFF)": "Software (XON/XOFF)",
        "Automatic": "Automatisch",
        "Backspace (^H)": "Backspace (^H)",
        "Delete (^?)": "Delete (^?)",
    },
    "fr": {
        "External RDP client": "Client RDP externe",
        "External VNC client": "Client VNC externe",
        "Quality (RemoteFX)": "Qualité (RemoteFX)",
        "Balanced (Adaptive)": "Équilibré (Adaptatif)",
        "Speed (Legacy)": "Vitesse (Legacy)",
        "Balanced": "Équilibré",
        "Speed": "Vitesse",
        "Auto (system)": "Auto (système)",
        "Odd": "Impaire",
        "Even": "Paire",
        "Hardware (RTS/CTS)": "Matériel (RTS/CTS)",
        "Software (XON/XOFF)": "Logiciel (XON/XOFF)",
        "Automatic": "Automatique",
        "Backspace (^H)": "Backspace (^H)",
        "Delete (^?)": "Delete (^?)",
    },
    "es": {
        "External RDP client": "Cliente RDP externo",
        "External VNC client": "Cliente VNC externo",
        "Quality (RemoteFX)": "Calidad (RemoteFX)",
        "Balanced (Adaptive)": "Equilibrado (Adaptativo)",
        "Speed (Legacy)": "Velocidad (Legacy)",
        "Balanced": "Equilibrado",
        "Speed": "Velocidad",
        "Auto (system)": "Auto (sistema)",
        "Odd": "Impar",
        "Even": "Par",
        "Hardware (RTS/CTS)": "Hardware (RTS/CTS)",
        "Software (XON/XOFF)": "Software (XON/XOFF)",
        "Automatic": "Automático",
        "Backspace (^H)": "Backspace (^H)",
        "Delete (^?)": "Delete (^?)",
    },
    "it": {
        "External RDP client": "Client RDP esterno",
        "External VNC client": "Client VNC esterno",
        "Quality (RemoteFX)": "Qualità (RemoteFX)",
        "Balanced (Adaptive)": "Bilanciato (Adattivo)",
        "Speed (Legacy)": "Velocità (Legacy)",
        "Balanced": "Bilanciato",
        "Speed": "Velocità",
        "Auto (system)": "Auto (sistema)",
        "Odd": "Dispari",
        "Even": "Pari",
        "Hardware (RTS/CTS)": "Hardware (RTS/CTS)",
        "Software (XON/XOFF)": "Software (XON/XOFF)",
        "Automatic": "Automatico",
        "Backspace (^H)": "Backspace (^H)",
        "Delete (^?)": "Delete (^?)",
    },
    "pl": {
        "External RDP client": "Zewnętrzny klient RDP",
        "External VNC client": "Zewnętrzny klient VNC",
        "Quality (RemoteFX)": "Jakość (RemoteFX)",
        "Balanced (Adaptive)": "Zrównoważony (Adaptacyjny)",
        "Speed (Legacy)": "Szybkość (Legacy)",
        "Balanced": "Zrównoważony",
        "Speed": "Szybkość",
        "Auto (system)": "Auto (systemowy)",
        "Odd": "Nieparzysta",
        "Even": "Parzysta",
        "Hardware (RTS/CTS)": "Sprzętowe (RTS/CTS)",
        "Software (XON/XOFF)": "Programowe (XON/XOFF)",
        "Automatic": "Automatycznie",
        "Backspace (^H)": "Backspace (^H)",
        "Delete (^?)": "Delete (^?)",
    },
    "cs": {
        "External RDP client": "Externí RDP klient",
        "External VNC client": "Externí VNC klient",
        "Quality (RemoteFX)": "Kvalita (RemoteFX)",
        "Balanced (Adaptive)": "Vyvážený (Adaptivní)",
        "Speed (Legacy)": "Rychlost (Legacy)",
        "Balanced": "Vyvážený",
        "Speed": "Rychlost",
        "Auto (system)": "Auto (systémový)",
        "Odd": "Lichá",
        "Even": "Sudá",
        "Hardware (RTS/CTS)": "Hardwarové (RTS/CTS)",
        "Software (XON/XOFF)": "Softwarové (XON/XOFF)",
        "Automatic": "Automaticky",
        "Backspace (^H)": "Backspace (^H)",
        "Delete (^?)": "Delete (^?)",
    },
    "sk": {
        "External RDP client": "Externý RDP klient",
        "External VNC client": "Externý VNC klient",
        "Quality (RemoteFX)": "Kvalita (RemoteFX)",
        "Balanced (Adaptive)": "Vyvážený (Adaptívny)",
        "Speed (Legacy)": "Rýchlosť (Legacy)",
        "Balanced": "Vyvážený",
        "Speed": "Rýchlosť",
        "Auto (system)": "Auto (systémový)",
        "Odd": "Nepárna",
        "Even": "Párna",
        "Hardware (RTS/CTS)": "Hardvérové (RTS/CTS)",
        "Software (XON/XOFF)": "Softvérové (XON/XOFF)",
        "Automatic": "Automaticky",
        "Backspace (^H)": "Backspace (^H)",
        "Delete (^?)": "Delete (^?)",
    },
    "da": {
        "External RDP client": "Ekstern RDP-klient",
        "External VNC client": "Ekstern VNC-klient",
        "Quality (RemoteFX)": "Kvalitet (RemoteFX)",
        "Balanced (Adaptive)": "Balanceret (Adaptiv)",
        "Speed (Legacy)": "Hastighed (Legacy)",
        "Balanced": "Balanceret",
        "Speed": "Hastighed",
        "Auto (system)": "Auto (system)",
        "Odd": "Ulige",
        "Even": "Lige",
        "Hardware (RTS/CTS)": "Hardware (RTS/CTS)",
        "Software (XON/XOFF)": "Software (XON/XOFF)",
        "Automatic": "Automatisk",
        "Backspace (^H)": "Backspace (^H)",
        "Delete (^?)": "Delete (^?)",
    },
    "sv": {
        "External RDP client": "Extern RDP-klient",
        "External VNC client": "Extern VNC-klient",
        "Quality (RemoteFX)": "Kvalitet (RemoteFX)",
        "Balanced (Adaptive)": "Balanserad (Adaptiv)",
        "Speed (Legacy)": "Hastighet (Legacy)",
        "Balanced": "Balanserad",
        "Speed": "Hastighet",
        "Auto (system)": "Auto (system)",
        "Odd": "Udda",
        "Even": "Jämn",
        "Hardware (RTS/CTS)": "Hårdvara (RTS/CTS)",
        "Software (XON/XOFF)": "Mjukvara (XON/XOFF)",
        "Automatic": "Automatisk",
        "Backspace (^H)": "Backspace (^H)",
        "Delete (^?)": "Delete (^?)",
    },
    "nl": {
        "External RDP client": "Externe RDP-client",
        "External VNC client": "Externe VNC-client",
        "Quality (RemoteFX)": "Kwaliteit (RemoteFX)",
        "Balanced (Adaptive)": "Gebalanceerd (Adaptief)",
        "Speed (Legacy)": "Snelheid (Legacy)",
        "Balanced": "Gebalanceerd",
        "Speed": "Snelheid",
        "Auto (system)": "Auto (systeem)",
        "Odd": "Oneven",
        "Even": "Even",
        "Hardware (RTS/CTS)": "Hardware (RTS/CTS)",
        "Software (XON/XOFF)": "Software (XON/XOFF)",
        "Automatic": "Automatisch",
        "Backspace (^H)": "Backspace (^H)",
        "Delete (^?)": "Delete (^?)",
    },
    "pt": {
        "External RDP client": "Cliente RDP externo",
        "External VNC client": "Cliente VNC externo",
        "Quality (RemoteFX)": "Qualidade (RemoteFX)",
        "Balanced (Adaptive)": "Equilibrado (Adaptativo)",
        "Speed (Legacy)": "Velocidade (Legacy)",
        "Balanced": "Equilibrado",
        "Speed": "Velocidade",
        "Auto (system)": "Auto (sistema)",
        "Odd": "Ímpar",
        "Even": "Par",
        "Hardware (RTS/CTS)": "Hardware (RTS/CTS)",
        "Software (XON/XOFF)": "Software (XON/XOFF)",
        "Automatic": "Automático",
        "Backspace (^H)": "Backspace (^H)",
        "Delete (^?)": "Delete (^?)",
    },
    "be": {
        "External RDP client": "Знешні RDP-кліент",
        "External VNC client": "Знешні VNC-кліент",
        "Quality (RemoteFX)": "Якасць (RemoteFX)",
        "Balanced (Adaptive)": "Збалансаваны (адаптыўны)",
        "Speed (Legacy)": "Хуткасць (састарэлы)",
        "Balanced": "Збалансаваны",
        "Speed": "Хуткасць",
        "Auto (system)": "Аўта (сістэмны)",
        "Odd": "Няцотная",
        "Even": "Цотная",
        "Hardware (RTS/CTS)": "Апаратнае (RTS/CTS)",
        "Software (XON/XOFF)": "Праграмнае (XON/XOFF)",
        "Automatic": "Аўтаматычна",
        "Backspace (^H)": "Backspace (^H)",
        "Delete (^?)": "Delete (^?)",
    },
    "kk": {
        "External RDP client": "Сыртқы RDP клиенті",
        "External VNC client": "Сыртқы VNC клиенті",
        "Quality (RemoteFX)": "Сапа (RemoteFX)",
        "Balanced (Adaptive)": "Теңдестірілген (Бейімделгіш)",
        "Speed (Legacy)": "Жылдамдық (Ескі)",
        "Balanced": "Теңдестірілген",
        "Speed": "Жылдамдық",
        "Auto (system)": "Авто (жүйелік)",
        "Odd": "Тақ",
        "Even": "Жұп",
        "Hardware (RTS/CTS)": "Аппараттық (RTS/CTS)",
        "Software (XON/XOFF)": "Бағдарламалық (XON/XOFF)",
        "Automatic": "Автоматты",
        "Backspace (^H)": "Backspace (^H)",
        "Delete (^?)": "Delete (^?)",
    },
    "uz": {
        "External RDP client": "Tashqi RDP mijoz",
        "External VNC client": "Tashqi VNC mijoz",
        "Quality (RemoteFX)": "Sifat (RemoteFX)",
        "Balanced (Adaptive)": "Muvozanatli (Moslashuvchan)",
        "Speed (Legacy)": "Tezlik (Eski)",
        "Balanced": "Muvozanatli",
        "Speed": "Tezlik",
        "Auto (system)": "Avto (tizim)",
        "Odd": "Toq",
        "Even": "Juft",
        "Hardware (RTS/CTS)": "Apparat (RTS/CTS)",
        "Software (XON/XOFF)": "Dasturiy (XON/XOFF)",
        "Automatic": "Avtomatik",
        "Backspace (^H)": "Backspace (^H)",
        "Delete (^?)": "Delete (^?)",
    },
}


def update_po_file(filepath, lang):
    """Update a .po file with translations for the given language."""
    if lang not in TRANSLATIONS:
        print(f"  No translations for {lang}, skipping")
        return 0

    with open(filepath, "r", encoding="utf-8") as f:
        content = f.read()

    trans = TRANSLATIONS[lang]
    count = 0

    for msgid, msgstr in trans.items():
        # Check if msgid already exists in the file
        escaped_msgid = msgid.replace("\\", "\\\\").replace('"', '\\"')
        pattern = re.compile(
            r'(msgid\s+"' + re.escape(escaped_msgid) + r'"\s*\n'
            r'msgstr\s+)"(.*?)"',
            re.DOTALL,
        )
        match = pattern.search(content)
        if match:
            existing = match.group(2)
            if existing == "":
                # Empty translation — fill it
                escaped_msgstr = msgstr.replace("\\", "\\\\").replace('"', '\\"')
                content = pattern.sub(r'\g<1>"' + escaped_msgstr + '"', content)
                count += 1
            else:
                pass  # Already translated
        else:
            # msgid not in file — append it
            escaped_msgid_po = msgid.replace("\\", "\\\\").replace('"', '\\"')
            escaped_msgstr = msgstr.replace("\\", "\\\\").replace('"', '\\"')
            entry = f'\nmsgid "{escaped_msgid_po}"\nmsgstr "{escaped_msgstr}"\n'
            content += entry
            count += 1

    with open(filepath, "w", encoding="utf-8") as f:
        f.write(content)

    return count


def main():
    po_dir = os.path.dirname(os.path.abspath(__file__))
    linguas_path = os.path.join(po_dir, "LINGUAS")

    with open(linguas_path, "r", encoding="utf-8") as f:
        languages = [
            line.strip()
            for line in f
            if line.strip() and not line.startswith("#")
        ]

    total = 0
    for lang in languages:
        po_file = os.path.join(po_dir, f"{lang}.po")
        if not os.path.exists(po_file):
            print(f"  {lang}.po not found, skipping")
            continue
        count = update_po_file(po_file, lang)
        print(f"  {lang}: {count} translations added/updated")
        total += count

    print(f"\nTotal: {total} translations across {len(languages)} languages")


if __name__ == "__main__":
    main()
