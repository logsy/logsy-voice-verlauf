//! Logsy Voice-Add-on: hebt die zuletzt diktierten Sätze auf.
//!
//! Es benutzt die beiden Teile des Add-on-Formats, die Inhalt tragen: das
//! Ereignis `transcript.final`, das als einziges den Text eines Diktats
//! mitbringt, und die eigene Seite in der Seitenleiste.
//!
//! # Das Gespräch
//!
//! ```text
//! ←  {"id":1,"event":"init","settings":{"anzahl":"20"}}
//! →  {"id":1,"ok":true}
//! ←  {"id":2,"event":"transcript.final","text":"Das war der Satz.","words":4,"settings":{…}}
//! →  {"id":2,"ok":true}
//! ←  {"id":3,"event":"view.render","settings":{…}}
//! →  {"id":3,"ok":true,"view":{"items":[…]}}
//! ←  {"id":4,"event":"view.action","action":"copy","item":"1","settings":{…}}
//! →  {"id":4,"ok":true,"view":{"items":[…]}}
//! ```
//!
//! # Wo die Sätze liegen
//!
//! Im Arbeitsspeicher dieses Prozesses und sonst nirgends. Das ist eine
//! Entscheidung, keine Auslassung: Ein Diktatverlauf auf der Platte überlebt
//! den Rechnerneustart, das Zurücksetzen des Programms und den nächsten
//! Benutzer — und niemand rechnet damit. Wer das Add-on ausschaltet, ist die
//! Sätze los.

mod zwischenablage;

use std::io::{BufRead, Write};
use std::time::Instant;

use serde_json::{Value, json};

/// Was ein Ereignis zurückgibt: nichts, oder die Seite.
type Ergebnis = Result<Option<Value>, String>;

/// Ein aufgehobenes Diktat.
struct Eintrag {
    /// Fortlaufend, damit eine Aktion sagen kann, welchen sie meint.
    ///
    /// Nicht die Stelle in der Liste: Die verschiebt sich beim nächsten
    /// Diktat, und dann löschte ein Klick den falschen Satz.
    nummer: u64,
    text: String,
    woerter: u64,
    empfangen: Instant,
}

/// Der ganze Zustand dieses Add-ons.
struct Verlauf {
    eintraege: Vec<Eintrag>,
    naechste: u64,
}

fn main() {
    let eingabe = std::io::stdin();
    let mut ausgabe = std::io::stdout();
    let mut verlauf = Verlauf { eintraege: Vec::new(), naechste: 1 };

    for zeile in eingabe.lock().lines() {
        // Ein Lesefehler heisst: Logsy Voice ist weg. Dann ist auch hier
        // Schluss — sonst bliebe dieses Programm als Waise im Speicher stehen.
        let Ok(zeile) = zeile else { break };
        let zeile = zeile.trim();
        if zeile.is_empty() {
            continue;
        }

        let Ok(nachricht) = serde_json::from_str::<Value>(zeile) else {
            eprintln!("Unverständliche Zeile übergangen");
            continue;
        };

        let id = nachricht.get("id").and_then(Value::as_u64).unwrap_or(0);
        let ereignis = nachricht.get("event").and_then(Value::as_str).unwrap_or("");

        let ergebnis = verlauf.behandle(ereignis, &nachricht);
        antworte(&mut ausgabe, id, ergebnis);

        if ereignis == "shutdown" {
            break;
        }
    }
}

impl Verlauf {
    fn behandle(&mut self, ereignis: &str, nachricht: &Value) -> Ergebnis {
        let einstellungen = nachricht.get("settings").cloned().unwrap_or(Value::Null);
        let hoechstens = anzahl(&einstellungen);

        match ereignis {
            "init" => Ok(None),

            "transcript.final" => {
                let text = nachricht.get("text").and_then(Value::as_str).unwrap_or("");
                if !text.trim().is_empty() {
                    self.aufnehmen(text, nachricht.get("words").and_then(Value::as_u64));
                    self.kuerzen(hoechstens);
                }
                Ok(None)
            }

            "view.render" => Ok(Some(self.seite())),

            "view.action" => {
                let aktion = nachricht.get("action").and_then(Value::as_str).unwrap_or("");
                let eintrag = nachricht.get("item").and_then(Value::as_str);
                self.ausfuehren(aktion, eintrag)?;
                // Die Antwort ist gleich die neue Seite. Logsy Voice fragt
                // danach nicht noch einmal.
                Ok(Some(self.seite()))
            }

            "shutdown" => Ok(None),

            // Ein unbekanntes Ereignis ist **kein Fehler**. Logsy Voice darf
            // neue einführen, ohne ältere Add-ons unbrauchbar zu machen.
            _ => Ok(None),
        }
    }

    /// Nimmt ein Diktat vorne in die Liste auf.
    fn aufnehmen(&mut self, text: &str, woerter: Option<u64>) {
        let nummer = self.naechste;
        self.naechste += 1;

        self.eintraege.insert(
            0,
            Eintrag {
                nummer,
                text: text.to_owned(),
                // Ohne Angabe selbst zählen: Ein älteres Logsy Voice schickt
                // das Feld vielleicht noch nicht mit.
                woerter: woerter.unwrap_or_else(|| text.split_whitespace().count() as u64),
                empfangen: Instant::now(),
            },
        );
    }

    /// Wirft hinten heraus, was über die eingestellte Anzahl hinausgeht.
    fn kuerzen(&mut self, hoechstens: usize) {
        self.eintraege.truncate(hoechstens);
    }

    /// Führt einen Klick aus.
    fn ausfuehren(&mut self, aktion: &str, eintrag: Option<&str>) -> Result<(), String> {
        match aktion {
            "copy" => {
                let nummer = kennung(eintrag)?;
                let gesucht = self
                    .eintraege
                    .iter()
                    .find(|e| e.nummer == nummer)
                    .ok_or("Diesen Satz gibt es nicht mehr.")?;
                zwischenablage::setzen(&gesucht.text)
            }

            "delete" => {
                let nummer = kennung(eintrag)?;
                self.eintraege.retain(|e| e.nummer != nummer);
                Ok(())
            }

            "clear" => {
                self.eintraege.clear();
                Ok(())
            }

            // Ein unbekannter Knopf kann nur von einem neueren Logsy Voice
            // kommen. Nichts zu tun ist richtiger, als etwas zu raten.
            _ => Ok(()),
        }
    }

    /// Die Seite, wie sie gerade aussieht.
    fn seite(&self) -> Value {
        let items: Vec<Value> = self
            .eintraege
            .iter()
            .map(|e| {
                json!({
                    "id": e.nummer.to_string(),
                    "title": wann(e.empfangen),
                    "text": e.text,
                    "meta": format!("{} {}", e.woerter, if e.woerter == 1 { "Wort" } else { "Wörter" }),
                    // Zeichen statt Wörter: An jedem Eintrag stünde sonst
                    // zweimal dasselbe Wortpaar, und das Auge liest es beim
                    // zwanzigsten Satz noch immer mit. Was sie tun, sagt der
                    // Text beim Darüberfahren — `label` ist dafür Pflicht.
                    "actions": [
                        { "id": "copy", "label": "In die Zwischenablage", "icon": "copy" },
                        { "id": "delete", "label": "Diesen Satz vergessen", "icon": "delete" },
                    ],
                })
            })
            .collect();

        let mut seite = json!({
            "note": "Diese Sätze liegen im Arbeitsspeicher. Wird das Add-on ausgeschaltet, sind sie weg.",
            "items": items,
        });

        // Der Knopf erscheint nur, wenn es etwas zu vergessen gibt. „Alles
        // vergessen" über einer leeren Liste wäre ein Knopf, der nichts tut.
        if !self.eintraege.is_empty() {
            seite["actions"] = json!([{ "id": "clear", "label": "Alles vergessen", "danger": true }]);
        }

        seite
    }
}

/// Die eingestellte Anzahl.
///
/// Die Auswahl liefert Zeichenketten — das Format kennt nur `boolean` und
/// `choice`, und eine Auswahl hat Werte, keine Zahlen.
fn anzahl(einstellungen: &Value) -> usize {
    einstellungen
        .get("anzahl")
        .and_then(Value::as_str)
        .and_then(|w| w.parse().ok())
        .unwrap_or(20)
}

/// Die Kennung aus einer Aktion.
fn kennung(eintrag: Option<&str>) -> Result<u64, String> {
    eintrag
        .and_then(|w| w.parse().ok())
        .ok_or_else(|| "Der Knopf sagt nicht, welchen Satz er meint.".to_owned())
}

/// Wie lange ein Eintrag her ist.
///
/// Grob und in Worten: Wer seinen letzten Satz sucht, will wissen, ob es der
/// von eben war — nicht, ob 94 oder 96 Sekunden vergangen sind.
fn wann(empfangen: Instant) -> String {
    let sekunden = empfangen.elapsed().as_secs();

    match sekunden {
        0..=59 => "gerade eben".to_owned(),
        60..=119 => "vor einer Minute".to_owned(),
        120..=3599 => format!("vor {} Minuten", sekunden / 60),
        3600..=7199 => "vor einer Stunde".to_owned(),
        _ => format!("vor {} Stunden", sekunden / 3600),
    }
}

/// Schreibt die Antwortzeile.
///
/// Auf jede empfangene Zeile gehört genau eine Antwort mit **derselben `id`**.
fn antworte(ausgabe: &mut impl Write, id: u64, ergebnis: Ergebnis) {
    let zeile = match ergebnis {
        Ok(None) => json!({ "id": id, "ok": true }),
        Ok(Some(seite)) => json!({ "id": id, "ok": true, "view": seite }),
        Err(grund) => json!({ "id": id, "ok": false, "error": grund }),
    };

    // Ohne `flush` bliebe die Antwort im Puffer, und Logsy Voice liefe in seine
    // Frist — bei jedem einzelnen Diktat.
    let _ = writeln!(ausgabe, "{zeile}");
    let _ = ausgabe.flush();
}
