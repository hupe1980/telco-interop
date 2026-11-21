//! Words and their sound: the name comparison of the AK S&P.
//!
//! Normalisation and tokens follow the „Infodokument WBCI Phonetische Prüfung" v1.1 §5:
//! words split at spaces and hyphens, the allowed special characters and digits become
//! spaces, case is ignored, words of one letter are ignored (so „L." never matches), and a
//! fixed list of stop words — legal forms, titles, salutations — is dropped. Each word is then
//! reduced to its Kölner Phonetik code (Postel 1969). The algorithm is an interpretation:
//! Anl. 1 V5.4.1 §2.4 leaves it „in der Zuständigkeit der einzelnen Häuser".

/// Stop words of the Infodokument v1.1 §5.3, after its normalisation (lower case, punctuation
/// as space, so „e.V." and „Z.Hd." leave only one-letter words or „hd").
const STOP_WORDS: &[&str] = &[
    "haftungsbeschränkt",
    "ag",
    "co",
    "kg",
    "gbr",
    "gmbh",
    "kgaa",
    "mbh",
    "ohg",
    "eg",
    "ev",
    "das",
    "der",
    "die",
    "und",
    "dipl",
    "dr",
    "med",
    "fr",
    "frau",
    "herr",
    "hr",
    "angabe",
    "anrede",
    "evangelisches",
    "firma",
    "hd",
    "hdn",
    "inh",
    "inhaber",
    "kath",
    "katholisches",
    "keine",
];

/// Upper-case ASCII letters for one character: umlauts and accented Latin-1 letters lose
/// their accent, `ß` becomes `SS`, everything else is dropped.
fn fold(c: char, out: &mut String) {
    let base = match c.to_ascii_uppercase() {
        c @ 'A'..='Z' => c,
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'A',
        'æ' | 'Æ' => {
            out.push_str("AE");
            return;
        }
        'ç' | 'Ç' => 'C',
        'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' | 'Ê' | 'Ë' => 'E',
        'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' | 'Î' | 'Ï' => 'I',
        'ð' | 'Ð' => 'D',
        'ñ' | 'Ñ' => 'N',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' => 'O',
        'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' | 'Û' | 'Ü' => 'U',
        'ý' | 'ÿ' | 'Ý' => 'Y',
        'þ' | 'Þ' => {
            out.push_str("TH");
            return;
        }
        'ß' => {
            out.push_str("SS");
            return;
        }
        _ => return,
    };
    out.push(base);
}

/// The Kölner Phonetik code of one word (Postel 1969): digits for sound classes, repeated
/// codes collapsed, vowels dropped except at the start. `Meier` and `Maier` both give `67`,
/// `Müller` and `Mueller` both `657`.
#[must_use]
pub fn koelner_phonetik(word: &str) -> String {
    let mut letters = String::new();
    for c in word.chars() {
        fold(c, &mut letters);
    }
    let l: Vec<char> = letters.chars().collect();
    let mut raw = String::new();
    for (i, &c) in l.iter().enumerate() {
        let prev = i.checked_sub(1).and_then(|p| l.get(p)).copied();
        let next = l.get(i + 1).copied();
        let code = match c {
            'A' | 'E' | 'I' | 'J' | 'O' | 'U' | 'Y' => "0",
            'B' => "1",
            'P' => {
                if next == Some('H') {
                    "3"
                } else {
                    "1"
                }
            }
            'D' | 'T' => {
                if matches!(next, Some('C' | 'S' | 'Z')) {
                    "8"
                } else {
                    "2"
                }
            }
            'F' | 'V' | 'W' => "3",
            'G' | 'K' | 'Q' => "4",
            'C' => {
                let hard_start = matches!(next, Some('A' | 'H' | 'K' | 'L' | 'O' | 'Q' | 'R' | 'U' | 'X'));
                let hard = matches!(next, Some('A' | 'H' | 'K' | 'O' | 'Q' | 'U' | 'X'));
                if i == 0 {
                    if hard_start { "4" } else { "8" }
                } else if matches!(prev, Some('S' | 'Z')) {
                    "8"
                } else if hard {
                    "4"
                } else {
                    "8"
                }
            }
            'X' => {
                if matches!(prev, Some('C' | 'K' | 'Q')) {
                    "8"
                } else {
                    "48"
                }
            }
            'L' => "5",
            'M' | 'N' => "6",
            'R' => "7",
            'S' | 'Z' => "8",
            _ => "",
        };
        raw.push_str(code);
    }
    let mut collapsed = String::new();
    for c in raw.chars() {
        if collapsed.ends_with(c) {
            continue;
        }
        collapsed.push(c);
    }
    collapsed.char_indices().filter(|&(i, c)| i == 0 || c != '0').map(|(_, c)| c).collect()
}

/// Splits a field into its words per the Infodokument: anything but a letter separates
/// words, one-letter words and stop words are dropped.
fn words(field: &str) -> impl Iterator<Item = String> + '_ {
    field
        .split(|c: char| !c.is_alphabetic())
        .filter(|w| w.chars().count() > 1)
        .map(str::to_lowercase)
        .filter(|w| !STOP_WORDS.contains(&w.as_str()))
}

/// The phonetic codes of a name field's words.
pub(crate) fn name_codes(field: &str) -> Vec<String> {
    words(field).map(|w| koelner_phonetik(&w)).filter(|c| !c.is_empty()).collect()
}

/// The phonetic codes of a street's words, with „Str."/„str" read as „Straße"
/// (an interpretation; the AK S&P documents do not normalise street names).
pub(crate) fn street_codes(field: &str) -> Vec<String> {
    field
        .split([' ', '-'])
        .map(|w| {
            let w = w.trim_end_matches('.').to_lowercase();
            match w.strip_suffix("str") {
                Some(stem) => format!("{stem}strasse"),
                None => w,
            }
        })
        .flat_map(|w| name_codes(&w))
        .collect()
}

/// Whether every word of `stored` (after normalisation) sounds like a word of `given`:
/// the Infodokument's rule „Alle Wörter aus Bestand müssen … in der Anfrage vorkommen".
/// Use it to search your records the way the check compares them.
#[must_use]
pub fn same_name(stored: &str, given: &str) -> bool {
    let given = name_codes(given);
    let stored = name_codes(stored);
    !stored.is_empty() && stored.iter().all(|c| given.contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn examples_of_the_specification() {
        assert_eq!(koelner_phonetik("Meier"), koelner_phonetik("Maier"));
        assert_eq!(koelner_phonetik("Müller"), "657");
        assert_eq!(koelner_phonetik("Mueller"), "657");
        assert_eq!(koelner_phonetik("Schmidt"), koelner_phonetik("Schmitt"));
        assert_ne!(koelner_phonetik("Schmidt"), koelner_phonetik("Schneider"));
        // Postel's own example.
        assert_eq!(koelner_phonetik("Müller-Lüdenscheidt"), "65752682");
        assert_eq!(koelner_phonetik("Wikipedia"), "3412");
        assert_eq!(koelner_phonetik(""), "");
    }

    #[test]
    fn words_follow_the_infodokument() {
        assert_eq!(name_codes("Hans-Jörg").len(), 2);
        assert_eq!(name_codes("L."), Vec::<String>::new());
        assert_eq!(name_codes("Dr. Erika"), name_codes("Erika"));
        assert!(same_name("Beispiel GmbH", "Beispiel G.m.b.H. & Co. KG"));
        assert!(same_name("Hans-Jörg Müller", "Hans Joerg Mueller"));
        assert!(!same_name("Syscomp Dienste", "Syscomp Dienstleistungen"));
        assert!(!same_name("Anna Marie", "AnnaMarie"));
        assert!(!same_name("Heinzpeter", "Heinz Peter"));
        assert!(!same_name("GmbH", "GmbH"), "a name of stop words only identifies nobody");
    }

    #[test]
    fn street_abbreviations() {
        assert_eq!(street_codes("Hauptstr."), street_codes("Hauptstraße"));
        assert_eq!(street_codes("Karl-Marx-Str. "), street_codes("Karl-Marx-Strasse"));
        assert_ne!(street_codes("Hauptstraße"), street_codes("Nebenstraße"));
    }
}
