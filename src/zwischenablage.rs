//! Text in die Windows-Zwischenablage legen.
//!
//! # Warum von Hand und nicht mit einer fertigen Kiste
//!
//! Die üblichen Kisten dafür bedienen Windows, macOS und X11 und bringen alles
//! mit, was dazugehört. Ein Logsy Voice-Add-on läuft unter Windows, und der Weg
//! dorthin sind die Zeilen hier — Öffnen, Leeren, einen Speicherblock füllen,
//! ihn übergeben, Schliessen.
//!
//! # Wem der Speicher gehört
//!
//! Nach `SetClipboardData` gehört der Block **Windows**, nicht mehr diesem
//! Programm. Ihn danach freizugeben wäre ein Fehler — deshalb steht hier
//! nirgends ein `GlobalFree`. Scheitert die Übergabe dagegen, gehört er noch
//! uns, und dann muss er weg.

use std::time::Duration;

use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};

/// Kennung für Unicode-Text in der Zwischenablage.
///
/// Von Hand statt aus `Win32_System_Ole`: Eine Zahl, die seit Windows 3.1
/// feststeht, ist kein Grund, ein weiteres Stück der Windows-Schnittstelle
/// mitzuübersetzen. Der Kern von Logsy Voice hält es genauso.
const CF_UNICODETEXT: u32 = 13;

/// Wie oft versucht wird, die Zwischenablage zu öffnen.
///
/// Sie gehört systemweit immer nur einem Prozess. Ein Passwortverwalter oder
/// ein Zwischenablage-Verlauf greift regelmässig zu; ein einzelner
/// Fehlversuch heisst also nicht, dass es nicht ginge — nur, dass gerade
/// jemand anderes dran war. Acht Versuche mit 15 ms Abstand bleiben mit
/// höchstens 105 ms weit innerhalb der Frist von 1,2 Sekunden.
const VERSUCHE: u32 = 8;

/// Pause zwischen den Versuchen.
const PAUSE: Duration = Duration::from_millis(15);

/// Legt Text in die Zwischenablage.
///
/// Ein `Err` landet als Bemerkung über der Liste und im Protokoll —
/// „Kopieren ging nicht" ohne Grund wäre keine Auskunft.
pub fn setzen(text: &str) -> Result<(), String> {
    // Windows erwartet UTF-16 mit abschliessender Null.
    let mut breit: Vec<u16> = text.encode_utf16().collect();
    breit.push(0);
    let bytes = std::mem::size_of_val(breit.as_slice());

    unsafe {
        oeffnen()?;

        let ergebnis = fuellen(&breit, bytes);

        // In jedem Fall schliessen. Eine offene Zwischenablage sperrt sie für
        // jedes andere Programm im System — ein vergessenes `CloseClipboard`
        // wäre der Fehler, den niemand mehr diesem Add-on zuordnet.
        let _ = CloseClipboard();
        ergebnis
    }
}

/// Öffnet die Zwischenablage, notfalls im zweiten Anlauf.
///
/// Ohne Fenster: Dieses Programm hat keines. Die Zwischenablage nimmt das an —
/// der Aufrufer wird dann dem aktuellen Vorgang zugeordnet.
unsafe fn oeffnen() -> Result<(), String> {
    let mut zuletzt = String::new();

    for versuch in 0..VERSUCHE {
        match unsafe { OpenClipboard(None) } {
            Ok(()) => return Ok(()),
            Err(e) => zuletzt = e.to_string(),
        }

        if versuch + 1 < VERSUCHE {
            std::thread::sleep(PAUSE);
        }
    }

    Err(format!("Die Zwischenablage war nicht zu öffnen: {zuletzt}"))
}

/// Der Teil zwischen Öffnen und Schliessen.
///
/// Ausgelagert, damit `CloseClipboard` genau einen Ort hat, an dem es steht,
/// und nicht hinter jedem `return` noch einmal.
unsafe fn fuellen(breit: &[u16], bytes: usize) -> Result<(), String> {
    unsafe {
        EmptyClipboard().map_err(|e| format!("Die Zwischenablage war nicht zu leeren: {e}"))?;

        let block: HGLOBAL = GlobalAlloc(GMEM_MOVEABLE, bytes)
            .map_err(|e| format!("Kein Speicher für die Zwischenablage: {e}"))?;

        let ziel = GlobalLock(block);
        if ziel.is_null() {
            let _ = GlobalFree(Some(block));
            return Err("Der Speicherblock liess sich nicht festhalten.".to_owned());
        }

        std::ptr::copy_nonoverlapping(breit.as_ptr(), ziel.cast::<u16>(), breit.len());
        let _ = GlobalUnlock(block);

        // Ab hier gehört der Block Windows — es sei denn, das hier schlägt fehl.
        SetClipboardData(CF_UNICODETEXT, Some(HANDLE(block.0))).map_err(|e| {
            let _ = GlobalFree(Some(block));
            format!("Die Zwischenablage nahm den Text nicht an: {e}")
        })?;

        Ok(())
    }
}
