use windows::Win32::System::SystemInformation::GetLocalTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Zeitpunkt {
    pub jahr: i64,
    pub monat: i64,
    pub tag: i64,
    pub stunde: u16,
    pub minute: u16,
}

pub fn jetzt() -> Zeitpunkt {
    let system = unsafe { GetLocalTime() };

    Zeitpunkt {
        jahr: i64::from(system.wYear),
        monat: i64::from(system.wMonth),
        tag: i64::from(system.wDay),
        stunde: system.wHour,
        minute: system.wMinute,
    }
}

impl Zeitpunkt {
    pub fn uhrzeit(&self) -> String {
        format!("{:02}:{:02}", self.stunde, self.minute)
    }

    /// Die Zahl der Tage seit dem 1. Januar 1970.
    ///
    /// Nach Howard Hinnants `days_from_civil` — gültig für jedes Datum des
    /// gregorianischen Kalenders und ohne Tabelle, die jemand pflegen müsste.
    pub fn tageszahl(&self) -> i64 {
        let jahr = if self.monat <= 2 { self.jahr - 1 } else { self.jahr };
        let aera = if jahr >= 0 { jahr } else { jahr - 399 } / 400;
        let jahr_der_aera = jahr - aera * 400;
        let monat = if self.monat > 2 { self.monat - 3 } else { self.monat + 9 };
        let tag_des_jahres = (153 * monat + 2) / 5 + self.tag - 1;
        let tag_der_aera =
            jahr_der_aera * 365 + jahr_der_aera / 4 - jahr_der_aera / 100 + tag_des_jahres;

        aera * 146_097 + tag_der_aera - 719_468
    }
}

/// Wann ein Eintrag entstanden ist, sobald „vor n Minuten" nichts mehr sagt.
pub fn beschriftung(dann: Zeitpunkt, jetzt: Zeitpunkt) -> String {
    match jetzt.tageszahl() - dann.tageszahl() {
        0 => dann.uhrzeit(),
        1 => format!("gestern, {}", dann.uhrzeit()),
        _ => format!("{}.{}., {}", dann.tag, dann.monat, dann.uhrzeit()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zeitpunkt(jahr: i64, monat: i64, tag: i64, stunde: u16, minute: u16) -> Zeitpunkt {
        Zeitpunkt { jahr, monat, tag, stunde, minute }
    }

    #[test]
    fn die_tageszahl_trifft_bekannte_daten() {
        assert_eq!(zeitpunkt(1970, 1, 1, 0, 0).tageszahl(), 0);
        assert_eq!(zeitpunkt(1969, 12, 31, 0, 0).tageszahl(), -1);
        assert_eq!(zeitpunkt(2000, 3, 1, 0, 0).tageszahl(), 11_017);
        assert_eq!(zeitpunkt(2026, 9, 7, 0, 0).tageszahl(), 20_703);
    }

    #[test]
    fn aufeinanderfolgende_tage_liegen_einen_auseinander() {
        // Über Monats-, Jahres- und Schaltjahresgrenze hinweg.
        let paare = [
            ((2026, 1, 31), (2026, 2, 1)),
            ((2025, 12, 31), (2026, 1, 1)),
            ((2024, 2, 28), (2024, 2, 29)),
            ((2024, 2, 29), (2024, 3, 1)),
            ((2100, 2, 28), (2100, 3, 1)),
        ];

        for ((j1, m1, t1), (j2, m2, t2)) in paare {
            let davor = zeitpunkt(j1, m1, t1, 0, 0).tageszahl();
            let danach = zeitpunkt(j2, m2, t2, 0, 0).tageszahl();
            assert_eq!(danach - davor, 1, "{j1}-{m1}-{t1} → {j2}-{m2}-{t2}");
        }
    }

    #[test]
    fn heute_ist_die_uhrzeit_gestern_bekommt_ein_wort() {
        let heute = zeitpunkt(2026, 9, 7, 9, 5);
        assert_eq!(beschriftung(zeitpunkt(2026, 9, 7, 14, 32), heute), "14:32");
        assert_eq!(beschriftung(zeitpunkt(2026, 9, 6, 8, 7), heute), "gestern, 08:07");
        assert_eq!(beschriftung(zeitpunkt(2026, 9, 3, 22, 0), heute), "3.9., 22:00");
    }

    #[test]
    fn eine_zurueckgestellte_uhr_zeigt_das_datum() {
        // Sommerzeit oder eine Zeitabgleichung darf keinen negativen Abstand
        // in „gestern" verwandeln.
        let heute = zeitpunkt(2026, 9, 7, 2, 30);
        assert_eq!(beschriftung(zeitpunkt(2026, 9, 8, 1, 0), heute), "8.9., 01:00");
    }
}
