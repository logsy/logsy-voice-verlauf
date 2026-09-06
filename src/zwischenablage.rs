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

use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_UNICODETEXT;

/// Legt Text in die Zwischenablage.
///
/// Ein `Err` landet als Grund in der Antwort an Logsy Voice und von dort in
/// der Oberfläche — „Kopieren ging nicht" ohne Grund wäre keine Auskunft.
pub fn setzen(text: &str) -> Result<(), String> {
    // Windows erwartet UTF-16 mit abschliessender Null.
    let mut breit: Vec<u16> = text.encode_utf16().collect();
    breit.push(0);
    let bytes = std::mem::size_of_val(breit.as_slice());

    unsafe {
        // Ohne Fenster: Dieses Programm hat keines. Die Zwischenablage nimmt
        // das an — der Aufrufer wird dann dem aktuellen Vorgang zugeordnet.
        OpenClipboard(None).map_err(|e| format!("Die Zwischenablage war nicht zu öffnen: {e}"))?;

        let ergebnis = fuellen(&breit, bytes);

        // In jedem Fall schliessen. Eine offene Zwischenablage sperrt sie für
        // jedes andere Programm im System — ein vergessenes `CloseClipboard`
        // wäre der Fehler, den niemand mehr diesem Add-on zuordnet.
        let _ = CloseClipboard();
        ergebnis
    }
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
        SetClipboardData(CF_UNICODETEXT.0.into(), Some(HANDLE(block.0))).map_err(|e| {
            let _ = GlobalFree(Some(block));
            format!("Die Zwischenablage nahm den Text nicht an: {e}")
        })?;

        Ok(())
    }
}
