![Logsy Voice-Add-on: Verlauf](https://cdn.reezy.dev/logsy/verlauf-addon.png)

# Verlauf

**Ein Add-on für [Logsy Voice](https://logsy.de).** Es hebt die zuletzt
diktierten Sätze auf und legt sie auf Klick in die Zwischenablage.

Diktieren geht schnell — und genau deshalb ist der Satz weg, sobald das
Zielfenster ihn verschluckt hat. Wer versehentlich ins falsche Fenster
gesprochen hat, wer den Text noch woanders braucht, wer nachsehen will, was
tatsächlich verstanden wurde: Der findet ihn hier.

Nach dem Einschalten steht **Letztes Diktat** in der Seitenleiste.

```
Letztes Diktat
──────────────────────────────────────────────────────────
Diese Sätze liegen im Arbeitsspeicher. Wird das Add-on
ausgeschaltet, sind sie weg.

  gerade eben                                ⧉   ✓   🗑
  Und das der zweite, mit Umlauten.
  8 Wörter

  14:32                                      ⧉   ✓   🗑
  Ein längeres Diktat über mehrere Zeilen.
  27 Wörter · angeheftet

                      [ Alles kopieren ]  [ Alles vergessen ]
```

## Installieren

In Logsy Voice unter **Integrationen › Add-ons** eintragen:

```
logsy/logsy-voice-verlauf
```

Logsy Voice holt die neueste Veröffentlichung, packt sie nach
`%APPDATA%\de.logsy.voice\addons\verlauf\` aus und startet sie beim
Einschalten. Mehr ist nicht zu tun.

## Was auf der Seite geht

| Knopf | Was er tut |
|---|---|
| **In die Zwischenablage** ⧉ | Legt diesen einen Satz hinein. |
| **Anheften** ✓ | Hält einen Satz fest. Angeheftetes zählt nicht zur eingestellten Anzahl und bleibt auch bei „Alles vergessen" stehen. |
| **Diesen Satz vergessen** 🗑 | Nimmt ihn aus der Liste. |
| **Alles kopieren** | Alle Sätze untereinander in die Zwischenablage. |
| **Zurücknehmen** | Holt zurück, was der letzte Löschvorgang weggenommen hat — jeden Satz an seine alte Stelle, bis zum nächsten Diktat. |
| **Alles vergessen** | Leert die Liste. Umkehrbar, solange nicht weitergesprochen wurde — deshalb fragt der Knopf nicht nach. |

Derselbe Satz zweimal hintereinander wird zu einem Eintrag mit „2×". Das ist
fast immer ein misslungenes Einfügen, das gerade wiederholt wurde, und zwei
gleiche Kacheln untereinander sähen aus wie ein Fehler.

Über jedem Satz steht in der ersten Stunde, wie lange er her ist, danach die
Uhrzeit — „vor 7 Stunden" sagt niemandem, wann das war. Ein Sprachkürzel
erscheint nur, wenn tatsächlich mehr als eine Sprache im Verlauf steht; sonst
stünde an jedem Eintrag dasselbe.

## Wo die Sätze liegen

**Im Arbeitsspeicher dieses Programms und sonst nirgends.**

Das ist eine Entscheidung, keine Auslassung. Ein Diktatverlauf auf der Platte
überlebt den Rechnerneustart, das Zurücksetzen der Anwendung und den nächsten
Benutzer — und niemand rechnet damit. Wer das Add-on ausschaltet oder
Logsy Voice beendet, ist die Sätze los. Das ist die Zusage.

Der Vollständigkeit halber gehören zwei Sätze dazu: Gelöschtes liegt für das
Zurücknehmen noch bis zum nächsten Diktat im Speicher. Und „nur im
Arbeitsspeicher" ist keine Zusage gegen Windows selbst — was ausgelagert wird,
entscheidet das Betriebssystem, nicht dieses Programm.

## Einstellungen

| Einstellung | Vorgabe | Bedeutung |
|---|---|---|
| **Wie viele aufgehoben werden** | 20 | 5, 20 oder 100. Alles darüber hinaus fällt hinten heraus. |

## Was es sieht

Dieses Add-on meldet `transcript.final` an — das einzige Ereignis von
Logsy Voice, das Text trägt. Es bekommt damit **jeden fertigen Satz**, den du
diktierst, nach Nachbearbeitung und Textbausteinen. Deshalb steht neben seiner
Kachel „Sieht deine Diktate", bevor jemand es einschaltet.

Was es damit **nicht** tut:

- Keine Netzverbindung. Nichts wird irgendwohin geschickt.
- Keine Datei. Nichts landet auf der Platte.
- Kein Protokoll ausser den Zeilen, die ohnehin im Protokoll von Logsy Voice
  stehen.

Wer nachsehen will: Es sind rund 900 Zeilen, davon knapp 300 Tests.

## Aufbau

| Datei | Inhalt |
|---|---|
| `src/main.rs` | Das Gespräch mit Logsy Voice und die Seite in der Oberfläche. |
| `src/zwischenablage.rs` | Das Kopieren, von Hand über die Win32-API. |
| `src/uhr.rs` | Die Ortszeit und die Rechnung „welcher Tag ist das". |
| `addon.json` | Das Beschreibungsblatt: Kennung, Fassung, Seite, Einstellungen. |

## Selbst bauen

```bash
cargo build --release
cargo test
```

Ausprobieren geht ohne Logsy Voice, indem man die Zeilen von Hand hineingibt —
das Add-on liest JSON von der Standardeingabe und antwortet auf derselben Zeile:

```bash
printf '%s\n%s\n' \
  '{"id":1,"event":"transcript.final","text":"Probe","words":1,"settings":{}}' \
  '{"id":2,"event":"view.render","settings":{}}' \
  | ./target/release/logsy-voice-verlauf.exe
```

Zurück kommen zwei Zeilen: eine Bestätigung und die fertige Seite.

## Veröffentlichen

Das Archiv baut GitHub, nicht dein Rechner. Fassung in `addon.json` hochzählen,
dann:

```bash
git tag v1.0.1 && git push --tags
```

Der Ablauf unter [`.github/workflows/release.yml`](.github/workflows/release.yml)
liest Kennung, Fassung, Programmdatei und Sinnbild aus dem Beschreibungsblatt,
bricht ab, wenn Fassung und Namensschild auseinandergehen, schnürt das Archiv
und hängt es an die Veröffentlichung.

## Voraussetzungen

Windows. Die Zwischenablage wird über die Win32-API angesprochen; einen
Ersatz für andere Systeme gibt es nicht, weil Logsy Voice selbst unter Windows
läuft.

## Lizenz

MIT. Siehe [LICENSE](LICENSE).
