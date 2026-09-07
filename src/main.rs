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

mod uhr;
mod zwischenablage;

use std::collections::VecDeque;
use std::io::{BufRead, ErrorKind, Write};
use std::time::Instant;

use serde_json::{Value, json};

/// Was ein Ereignis zurückgibt: nichts, oder die Seite.
type Ergebnis = Result<Option<Value>, String>;

/// Der Satz über der Liste.
const BEMERKUNG: &str = "Diese Sätze liegen im Arbeitsspeicher. Wird das Add-on ausgeschaltet, sind sie weg.";

/// Ein aufgehobenes Diktat.
struct Eintrag {
    /// Fortlaufend, damit eine Aktion sagen kann, welchen sie meint.
    ///
    /// Nicht die Stelle in der Liste: Die verschiebt sich beim nächsten
    /// Diktat, und dann löschte ein Klick den falschen Satz.
    nummer: u64,
    text: String,
    woerter: u64,
    sprache: Option<String>,
    /// Für den Abstand — eine monotone Uhr, die kein Zeitabgleich verstellt.
    empfangen: Instant,
    /// Für die Anzeige, sobald der Abstand nichts mehr sagt.
    uhrzeit: uhr::Zeitpunkt,
    /// Wie oft derselbe Satz hintereinander kam.
    male: u32,
    angeheftet: bool,
}

/// Der ganze Zustand dieses Add-ons.
struct Verlauf {
    eintraege: VecDeque<Eintrag>,
    naechste: u64,
    /// Was der letzte Löschvorgang weggenommen hat, samt seiner Stelle.
    zurueckgelegt: Vec<(usize, Eintrag)>,
}

fn main() {
    let eingabe = std::io::stdin();
    let mut ausgabe = std::io::stdout();
    let mut verlauf = Verlauf::neu();

    for zeile in eingabe.lock().lines() {
        let zeile = match zeile {
            Ok(zeile) => zeile,
            // Ungültige Zeichen sind eine kaputte Zeile, nicht das Ende des
            // Gesprächs. Sie kostet ihre Antwort, mehr nicht.
            Err(fehler) if fehler.kind() == ErrorKind::InvalidData => {
                eprintln!("Zeile mit ungültigen Zeichen übergangen");
                continue;
            }
            // Jeder andere Lesefehler heisst: Logsy Voice ist weg. Dann ist
            // auch hier Schluss — sonst bliebe dieses Programm als Waise im
            // Speicher stehen.
            Err(_) => break,
        };

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
    fn neu() -> Self {
        Self { eintraege: VecDeque::new(), naechste: 1, zurueckgelegt: Vec::new() }
    }

    fn behandle(&mut self, ereignis: &str, nachricht: &Value) -> Ergebnis {
        let einstellungen = nachricht.get("settings").unwrap_or(&Value::Null);
        let hoechstens = anzahl(einstellungen);

        match ereignis {
            "init" => Ok(None),

            "transcript.final" => {
                let text = nachricht.get("text").and_then(Value::as_str).unwrap_or("");
                if !text.trim().is_empty() {
                    self.aufnehmen(
                        text,
                        nachricht.get("words").and_then(Value::as_u64),
                        nachricht.get("language").and_then(Value::as_str),
                    );
                    self.kuerzen(hoechstens);
                    // Ein Zurücknehmen, das ein Diktat überdauert, wäre ein
                    // Knopf, an dessen Wirkung sich niemand mehr erinnert.
                    self.zurueckgelegt.clear();
                }
                Ok(None)
            }

            // Auch hier gekürzt, nicht nur beim Diktat: Sonst zeigte die Seite
            // nach einer verkleinerten Anzahl weiter den alten Stand — bis zum
            // nächsten Mal, das Stunden später kommen kann.
            "view.render" => {
                self.kuerzen(hoechstens);
                Ok(Some(self.seite(None)))
            }

            "view.action" => {
                let aktion = nachricht.get("action").and_then(Value::as_str).unwrap_or("");
                let eintrag = nachricht.get("item").and_then(Value::as_str);
                let hinweis = self.ausfuehren(aktion, eintrag);
                self.kuerzen(hoechstens);
                // Die Antwort ist gleich die neue Seite. Logsy Voice fragt
                // danach nicht noch einmal.
                Ok(Some(self.seite(hinweis)))
            }

            "shutdown" => Ok(None),

            // Ein unbekanntes Ereignis ist **kein Fehler**. Logsy Voice darf
            // neue einführen, ohne ältere Add-ons unbrauchbar zu machen.
            _ => Ok(None),
        }
    }

    /// Nimmt ein Diktat vorne in die Liste auf.
    fn aufnehmen(&mut self, text: &str, woerter: Option<u64>, sprache: Option<&str>) {
        // Zweimal derselbe Satz ist fast immer ein misslungenes Einfügen, das
        // gerade wiederholt wurde. Zwei gleiche Kacheln untereinander sähen
        // aus wie ein Fehler des Add-ons.
        if let Some(erster) = self.eintraege.front_mut() {
            if erster.text == text {
                erster.male += 1;
                erster.empfangen = Instant::now();
                erster.uhrzeit = uhr::jetzt();
                return;
            }
        }

        let nummer = self.naechste;
        self.naechste += 1;

        self.eintraege.push_front(Eintrag {
            nummer,
            text: text.to_owned(),
            // Ohne Angabe selbst zählen: Ein älteres Logsy Voice schickt
            // das Feld vielleicht noch nicht mit.
            woerter: woerter.unwrap_or_else(|| text.split_whitespace().count() as u64),
            sprache: sprache.map(str::to_owned),
            empfangen: Instant::now(),
            uhrzeit: uhr::jetzt(),
            male: 1,
            angeheftet: false,
        });
    }

    /// Wirft hinten heraus, was über die eingestellte Anzahl hinausgeht.
    ///
    /// Angeheftetes zählt nicht mit und bleibt. Wer einen Satz festhält, will
    /// ihn nicht dadurch verlieren, dass er weiterdiktiert.
    fn kuerzen(&mut self, hoechstens: usize) {
        let mut frei = 0;
        self.eintraege.retain(|e| {
            if e.angeheftet {
                return true;
            }
            frei += 1;
            frei <= hoechstens
        });
    }

    /// Führt einen Klick aus und gibt zurück, was dabei schiefging.
    ///
    /// Ein `Err` an Logsy Voice zurückzugeben hiesse, die Seite gegen eine
    /// Fehlermeldung einzutauschen — der Nutzer verlöre den Blick auf seine
    /// Sätze, weil das Kopieren eines einzelnen nicht ging. Der Grund steht
    /// deshalb über der Liste und zusätzlich im Protokoll.
    fn ausfuehren(&mut self, aktion: &str, eintrag: Option<&str>) -> Option<String> {
        let grund = self.versuchen(aktion, eintrag).err()?;
        eprintln!("{grund}");
        Some(grund)
    }

    fn versuchen(&mut self, aktion: &str, eintrag: Option<&str>) -> Result<(), String> {
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

            "copy-all" => {
                if self.eintraege.is_empty() {
                    return Ok(());
                }
                let alles: Vec<&str> = self.eintraege.iter().map(|e| e.text.as_str()).collect();
                zwischenablage::setzen(&alles.join("\n\n"))
            }

            "pin" => {
                let nummer = kennung(eintrag)?;
                let gesucht = self
                    .eintraege
                    .iter_mut()
                    .find(|e| e.nummer == nummer)
                    .ok_or("Diesen Satz gibt es nicht mehr.")?;
                gesucht.angeheftet = !gesucht.angeheftet;
                Ok(())
            }

            "delete" => {
                let nummer = kennung(eintrag)?;
                let Some(stelle) = self.eintraege.iter().position(|e| e.nummer == nummer) else {
                    return Ok(());
                };
                if let Some(weg) = self.eintraege.remove(stelle) {
                    self.zurueckgelegt = vec![(stelle, weg)];
                }
                Ok(())
            }

            "clear" => {
                let mut zurueck = Vec::new();
                let mut bleibt = VecDeque::new();

                for (stelle, e) in std::mem::take(&mut self.eintraege).into_iter().enumerate() {
                    if e.angeheftet {
                        bleibt.push_back(e);
                    } else {
                        zurueck.push((stelle, e));
                    }
                }

                self.eintraege = bleibt;
                if !zurueck.is_empty() {
                    self.zurueckgelegt = zurueck;
                }
                Ok(())
            }

            // Aufsteigend eingesetzt landet jeder Satz wieder an seiner alten
            // Stelle: Alles, was vor ihm stand, ist dann schon wieder da.
            "undo" => {
                for (stelle, e) in std::mem::take(&mut self.zurueckgelegt) {
                    let stelle = stelle.min(self.eintraege.len());
                    self.eintraege.insert(stelle, e);
                }
                Ok(())
            }

            // Ein unbekannter Knopf kann nur von einem neueren Logsy Voice
            // kommen. Nichts zu tun ist richtiger, als etwas zu raten.
            _ => Ok(()),
        }
    }

    /// Die Seite, wie sie gerade aussieht.
    fn seite(&self, hinweis: Option<String>) -> Value {
        let jetzt = uhr::jetzt();
        let mit_sprache = self.mehrsprachig();

        let items: Vec<Value> = self
            .eintraege
            .iter()
            .map(|e| {
                let anheften = if e.angeheftet { "Nicht mehr anheften" } else { "Anheften" };

                json!({
                    "id": e.nummer.to_string(),
                    "title": wann(e.empfangen.elapsed().as_secs(), e.uhrzeit, jetzt),
                    "text": e.text,
                    "meta": meta(e, mit_sprache),
                    // Zeichen statt Wörter: An jedem Eintrag stünde sonst
                    // dreimal dasselbe Wortpaar, und das Auge liest es beim
                    // zwanzigsten Satz noch immer mit. Was sie tun, sagt der
                    // Text beim Darüberfahren — `label` ist dafür Pflicht.
                    "actions": [
                        { "id": "copy", "label": "In die Zwischenablage", "icon": "copy" },
                        { "id": "pin", "label": anheften, "icon": "check" },
                        { "id": "delete", "label": "Diesen Satz vergessen", "icon": "delete" },
                    ],
                })
            })
            .collect();

        let mut seite = json!({ "items": items });

        // Über einer leeren Liste stünde sonst ein Satz über Sätze, die es
        // nicht gibt — direkt über dem `empty` aus dem Beschreibungsblatt.
        if let Some(note) = hinweis.or_else(|| {
            (!self.eintraege.is_empty()).then(|| BEMERKUNG.to_owned())
        }) {
            seite["note"] = json!(note);
        }

        // Jeder Knopf erscheint nur, wenn er etwas zu tun hätte. „Alles
        // vergessen" über einer leeren Liste wäre ein Knopf, der nichts tut.
        //
        // Und hier ohne `icon`, anders als in der Zeile: Ein Zeichen ersetzt
        // das Wort, und ein einzelnes Quadrat unter der Liste sagt niemandem,
        // was es kopiert. In der Zeile trägt der Zusammenhang es, unter der
        // Liste nicht.
        let mut aktionen = Vec::new();
        if !self.eintraege.is_empty() {
            aktionen.push(json!({ "id": "copy-all", "label": "Alles kopieren" }));
        }
        if !self.zurueckgelegt.is_empty() {
            aktionen.push(json!({ "id": "undo", "label": "Zurücknehmen" }));
        }
        if self.eintraege.iter().any(|e| !e.angeheftet) {
            aktionen.push(json!({ "id": "clear", "label": "Alles vergessen", "danger": true }));
        }
        if !aktionen.is_empty() {
            seite["actions"] = json!(aktionen);
        }

        seite
    }

    /// Ob überhaupt mehr als eine Sprache im Verlauf steht.
    ///
    /// Nur dann sagt ein Kürzel an jedem Eintrag etwas. Wer immer deutsch
    /// diktiert, bekommt sonst zwanzigmal „DE" untereinander.
    fn mehrsprachig(&self) -> bool {
        let mut gesehen: Option<&str> = None;

        for sprache in self.eintraege.iter().filter_map(|e| e.sprache.as_deref()) {
            match gesehen {
                None => gesehen = Some(sprache),
                Some(erste) if erste != sprache => return true,
                _ => {}
            }
        }

        false
    }
}

/// Die zweite Zeile eines Eintrags.
fn meta(e: &Eintrag, mit_sprache: bool) -> String {
    let mut teile =
        vec![format!("{} {}", e.woerter, if e.woerter == 1 { "Wort" } else { "Wörter" })];

    if e.male > 1 {
        teile.push(format!("{}×", e.male));
    }
    if mit_sprache {
        if let Some(sprache) = &e.sprache {
            teile.push(sprache.to_uppercase());
        }
    }
    // Ohne das sähe ein angehefteter Satz aus wie jeder andere — das Format
    // kennt keinen hervorgehobenen Eintrag, nur Text.
    if e.angeheftet {
        teile.push("angeheftet".to_owned());
    }

    teile.join(" · ")
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
/// In der ersten Stunde grob und in Worten: Wer seinen letzten Satz sucht,
/// will wissen, ob es der von eben war — nicht, ob 94 oder 96 Sekunden
/// vergangen sind. Danach kippt es auf die Uhrzeit, weil „vor 7 Stunden"
/// niemandem mehr sagt, wann das war.
///
/// Der Abstand kommt herein, statt hier aus dem Eintrag gelesen zu werden:
/// So lässt sich jede Stufe prüfen, ohne eine Uhr zu verstellen, die sich
/// nicht verstellen lässt.
fn wann(sekunden: u64, uhrzeit: uhr::Zeitpunkt, jetzt: uhr::Zeitpunkt) -> String {
    match sekunden {
        0..=59 => "gerade eben".to_owned(),
        60..=119 => "vor einer Minute".to_owned(),
        120..=3599 => format!("vor {} Minuten", sekunden / 60),
        _ => uhr::beschriftung(uhrzeit, jetzt),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Ein Diktat, wie Logsy Voice es schickt.
    fn diktat(text: &str) -> Value {
        json!({ "id": 1, "event": "transcript.final", "text": text, "words": 3, "settings": {} })
    }

    fn klick(aktion: &str, eintrag: Option<&str>) -> Value {
        json!({ "id": 2, "event": "view.action", "action": aktion, "item": eintrag, "settings": {} })
    }

    fn seite(verlauf: &mut Verlauf, anzahl: &str) -> Value {
        let frage = json!({ "id": 3, "event": "view.render", "settings": { "anzahl": anzahl } });
        verlauf
            .behandle("view.render", &frage)
            .expect("view.render antwortet nie mit einem Fehler")
            .expect("view.render liefert immer eine Seite")
    }

    /// Die Texte der Einträge in der Reihenfolge, in der sie dastehen.
    fn texte(seite: &Value) -> Vec<String> {
        seite["items"]
            .as_array()
            .expect("items ist eine Liste")
            .iter()
            .map(|e| e["text"].as_str().expect("text ist eine Zeichenkette").to_owned())
            .collect()
    }

    fn knoepfe(seite: &Value) -> Vec<String> {
        seite["actions"]
            .as_array()
            .map(|liste| {
                liste
                    .iter()
                    .map(|a| a["id"].as_str().unwrap_or_default().to_owned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Ein Verlauf mit `n` Diktaten, das jüngste zuerst.
    fn gefuellt(n: usize) -> Verlauf {
        let mut verlauf = Verlauf::neu();
        for i in 0..n {
            verlauf.behandle("transcript.final", &diktat(&format!("Satz {i}"))).expect("ok");
        }
        verlauf
    }

    #[test]
    fn ein_leeres_diktat_wird_nicht_aufgehoben() {
        let mut verlauf = Verlauf::neu();
        verlauf.behandle("transcript.final", &diktat("   ")).expect("ok");
        assert!(verlauf.eintraege.is_empty());
    }

    #[test]
    fn dasselbe_zweimal_wird_zu_einem_eintrag() {
        let mut verlauf = Verlauf::neu();
        verlauf.behandle("transcript.final", &diktat("Nochmal")).expect("ok");
        verlauf.behandle("transcript.final", &diktat("Nochmal")).expect("ok");

        assert_eq!(verlauf.eintraege.len(), 1);
        assert_eq!(verlauf.eintraege[0].male, 2);
        assert!(seite(&mut verlauf, "20")["items"][0]["meta"]
            .as_str()
            .expect("meta steht da")
            .contains("2×"));
    }

    #[test]
    fn dazwischen_gesprochenes_trennt_die_beiden() {
        let mut verlauf = Verlauf::neu();
        verlauf.behandle("transcript.final", &diktat("A")).expect("ok");
        verlauf.behandle("transcript.final", &diktat("B")).expect("ok");
        verlauf.behandle("transcript.final", &diktat("A")).expect("ok");

        assert_eq!(texte(&seite(&mut verlauf, "20")), ["A", "B", "A"]);
    }

    #[test]
    fn eine_verkleinerte_anzahl_wirkt_sofort() {
        // Ohne das zeigte die Seite bis zum nächsten Diktat den alten Stand —
        // und das kann Stunden später sein.
        let mut verlauf = gefuellt(7);
        assert_eq!(texte(&seite(&mut verlauf, "5")).len(), 5);
    }

    #[test]
    fn angeheftetes_ueberlebt_die_anzahl() {
        let mut verlauf = gefuellt(3);
        let aeltester = verlauf.eintraege[2].nummer.to_string();
        verlauf.behandle("view.action", &klick("pin", Some(&aeltester))).expect("ok");

        let stand = seite(&mut verlauf, "1");
        assert_eq!(texte(&stand), ["Satz 2", "Satz 0"]);
        assert!(stand["items"][1]["meta"].as_str().expect("meta").contains("angeheftet"));
    }

    #[test]
    fn ein_geloeschter_satz_kommt_an_seine_stelle_zurueck() {
        let mut verlauf = gefuellt(3);
        let mittlerer = verlauf.eintraege[1].nummer.to_string();

        verlauf.behandle("view.action", &klick("delete", Some(&mittlerer))).expect("ok");
        assert_eq!(texte(&seite(&mut verlauf, "20")), ["Satz 2", "Satz 0"]);

        verlauf.behandle("view.action", &klick("undo", None)).expect("ok");
        assert_eq!(texte(&seite(&mut verlauf, "20")), ["Satz 2", "Satz 1", "Satz 0"]);
    }

    #[test]
    fn alles_vergessen_laesst_angeheftetes_stehen_und_ist_umkehrbar() {
        let mut verlauf = gefuellt(3);
        let mittlerer = verlauf.eintraege[1].nummer.to_string();
        verlauf.behandle("view.action", &klick("pin", Some(&mittlerer))).expect("ok");

        verlauf.behandle("view.action", &klick("clear", None)).expect("ok");
        assert_eq!(texte(&seite(&mut verlauf, "20")), ["Satz 1"]);

        verlauf.behandle("view.action", &klick("undo", None)).expect("ok");
        assert_eq!(texte(&seite(&mut verlauf, "20")), ["Satz 2", "Satz 1", "Satz 0"]);
    }

    #[test]
    fn zuruecknehmen_verfaellt_mit_dem_naechsten_diktat() {
        let mut verlauf = gefuellt(2);
        verlauf.behandle("view.action", &klick("clear", None)).expect("ok");
        assert!(knoepfe(&seite(&mut verlauf, "20")).contains(&"undo".to_owned()));

        verlauf.behandle("transcript.final", &diktat("Weiter")).expect("ok");
        assert!(!knoepfe(&seite(&mut verlauf, "20")).contains(&"undo".to_owned()));
    }

    #[test]
    fn ueber_einer_leeren_liste_steht_keine_bemerkung() {
        let mut verlauf = Verlauf::neu();
        let leer = seite(&mut verlauf, "20");
        assert!(leer.get("note").is_none());
        assert!(leer.get("actions").is_none());

        verlauf.behandle("transcript.final", &diktat("Etwas")).expect("ok");
        assert!(seite(&mut verlauf, "20")["note"].is_string());
    }

    #[test]
    fn die_sprache_steht_nur_da_wenn_sie_wechselt() {
        let mut verlauf = Verlauf::neu();
        let mut deutsch = diktat("Guten Tag");
        deutsch["language"] = json!("de");
        verlauf.behandle("transcript.final", &deutsch).expect("ok");

        let nur_deutsch = seite(&mut verlauf, "20");
        assert_eq!(nur_deutsch["items"][0]["meta"], json!("3 Wörter"));

        let mut englisch = diktat("Good morning");
        englisch["language"] = json!("en");
        verlauf.behandle("transcript.final", &englisch).expect("ok");

        let gemischt = seite(&mut verlauf, "20");
        assert_eq!(gemischt["items"][0]["meta"], json!("3 Wörter · EN"));
        assert_eq!(gemischt["items"][1]["meta"], json!("3 Wörter · DE"));
    }

    #[test]
    fn ein_unbekanntes_ereignis_ist_kein_fehler() {
        let mut verlauf = Verlauf::neu();
        let nachricht = json!({ "id": 1, "event": "was.neues", "settings": {} });
        assert_eq!(verlauf.behandle("was.neues", &nachricht), Ok(None));
    }

    #[test]
    fn ein_unbekannter_knopf_aendert_nichts_und_kostet_die_seite_nicht() {
        let mut verlauf = gefuellt(2);
        let antwort = verlauf.behandle("view.action", &klick("rakete", None)).expect("ok");

        let stand = antwort.expect("auch darauf kommt eine Seite");
        assert_eq!(texte(&stand), ["Satz 1", "Satz 0"]);
        assert_eq!(stand["note"], json!(BEMERKUNG));
    }

    #[test]
    fn ein_klick_auf_einen_verschwundenen_satz_kostet_die_liste_nicht() {
        // Der Grund gehört über die Liste, nicht an ihre Stelle.
        let mut verlauf = gefuellt(2);
        let stand = verlauf
            .behandle("view.action", &klick("copy", Some("999")))
            .expect("kein Fehler an Logsy Voice")
            .expect("eine Seite");

        assert_eq!(stand["note"], json!("Diesen Satz gibt es nicht mehr."));
        assert_eq!(texte(&stand).len(), 2);
    }

    #[test]
    fn ohne_angabe_gilt_zwanzig() {
        assert_eq!(anzahl(&json!({})), 20);
        assert_eq!(anzahl(&Value::Null), 20);
        assert_eq!(anzahl(&json!({ "anzahl": "5" })), 5);
        // Eine Zahl statt einer Zeichenkette käme von einem Logsy Voice, das
        // das Format anders liest, als es festgeschrieben ist.
        assert_eq!(anzahl(&json!({ "anzahl": 5 })), 20);
    }

    #[test]
    fn eine_aktion_ohne_eintrag_sagt_warum() {
        assert_eq!(kennung(Some("7")), Ok(7));
        assert!(kennung(None).is_err());
        assert!(kennung(Some("keine Zahl")).is_err());
    }

    #[test]
    fn der_abstand_wird_zur_uhrzeit() {
        let heute = uhr::Zeitpunkt { jahr: 2026, monat: 9, tag: 7, stunde: 18, minute: 0 };
        let vorhin = uhr::Zeitpunkt { jahr: 2026, monat: 9, tag: 7, stunde: 9, minute: 5 };

        assert_eq!(wann(0, vorhin, heute), "gerade eben");
        assert_eq!(wann(59, vorhin, heute), "gerade eben");
        assert_eq!(wann(90, vorhin, heute), "vor einer Minute");
        assert_eq!(wann(300, vorhin, heute), "vor 5 Minuten");
        assert_eq!(wann(3599, vorhin, heute), "vor 59 Minuten");
        // Ab hier sagt der Abstand nichts mehr, also sagt es die Uhr.
        assert_eq!(wann(3600, vorhin, heute), "09:05");
        assert_eq!(wann(9 * 3600, vorhin, heute), "09:05");
    }
}
