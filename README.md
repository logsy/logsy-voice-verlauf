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

  gerade eben                                    ⧉   🗑
  Und das der zweite, mit Umlauten.
  8 Wörter

  vor 3 Minuten                                  ⧉   🗑
  Ein längeres Diktat über mehrere Zeilen.
  27 Wörter

                                    [ Alles vergessen ]
```

## Installieren

In Logsy Voice unter **Integrationen › Add-ons** eintragen:

```
reezy-development/logsy-voice-verlauf
```

Logsy Voice holt die neueste Veröffentlichung, packt sie nach
`%APPDATA%\de.logsy.voice\addons\verlauf\` aus und startet sie beim
Einschalten. Mehr ist nicht zu tun.

## Wo die Sätze liegen

**Im Arbeitsspeicher dieses Programms und sonst nirgends.**

Das ist eine Entscheidung, keine Auslassung. Ein Diktatverlauf auf der Platte
überlebt den Rechnerneustart, das Zurücksetzen der Anwendung und den nächsten
Benutzer — und niemand rechnet damit. Wer das Add-on ausschaltet oder
Logsy Voice beendet, ist die Sätze los. Das ist die Zusage.

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

Wer nachsehen will: Es sind rund 250 Zeilen, und `src/main.rs` enthält alles
ausser dem Windows-Teil für die Zwischenablage.

## Aufbau

| Datei | Inhalt |
|---|---|
| `src/main.rs` | Das Gespräch mit Logsy Voice und die Seite in der Oberfläche. |
| `src/zwischenablage.rs` | Alles, was mit Windows zu tun hat. |
| `addon.json` | Das Beschreibungsblatt: Kennung, Fassung, Seite, Einstellungen. |

## Selbst bauen

```bash
cargo build --release
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
