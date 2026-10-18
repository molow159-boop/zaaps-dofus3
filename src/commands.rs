use crate::data::{normalize, Zaap};
use regex::Regex;
use std::{
    collections::HashSet,
    sync::OnceLock,
    time::{Duration, Instant},
};

const MAX_AGE: Duration = Duration::from_secs(4);

#[derive(Clone, Default)]
pub struct Step {
    pub zaap: Option<String>,
    pub travel: Option<String>,
    pub error: Option<String>,
    pub updated: Option<Instant>,
    pub identity: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub enum Action {
    Zaap,
    Travel,
    Combined,
}

impl Step {
    pub fn from_text(text: &str, known: &[Zaap]) -> Self {
        static PROGRESS: OnceLock<Regex> = OnceLock::new();
        let progress = PROGRESS.get_or_init(|| {
            Regex::new(r"\b(\d{1,5})\s*/\s*(\d{1,5})\b").expect("expression régulière valide")
        });
        let counters: HashSet<(u32, u32)> = progress
            .captures_iter(text)
            .filter_map(|c| {
                let current = c[1].parse().ok()?;
                let total = c[2].parse().ok()?;
                (current > 0 && current <= total).then_some((current, total))
            })
            .collect();
        if let Some(max_total) = counters.iter().map(|(_, total)| total).max() {
            if counters
                .iter()
                .filter(|(_, total)| total == max_total)
                .count()
                > 1
            {
                return Self::unavailable("Plusieurs étapes visibles : lecture ambiguë");
            }
        }
        // Coordinates in a dimension do not establish a route in the current world.
        let normalized = normalize(text);
        if ["xelorium", "dimension", "enutrosor", "srambad", "ecaflipus"]
            .iter()
            .any(|name| normalized.contains(name))
        {
            return Self {
                identity: step_identity(text, None, None),
                ..Self::unavailable("Destination dimensionnelle : trajet non vérifiable")
            };
        }
        let zaap = recommended_zaap(text, known).map(|z| z.command());
        static ZAAP_MENTION: OnceLock<Regex> = OnceLock::new();
        if zaap.is_none()
            && ZAAP_MENTION
                .get_or_init(|| Regex::new(r"(?i)\bzaap\b").expect("expression régulière valide"))
                .is_match(text)
        {
            return Self {
                identity: step_identity(text, None, None),
                ..Self::unavailable("Zaap recommandé absent ou ambigu")
            };
        }
        let travel = travel_destination(text).map(|(x, y)| format!("/travel {} {}", x, y));
        let identity = step_identity(text, zaap.as_deref(), travel.as_deref());
        Self {
            zaap,
            travel,
            error: None,
            updated: Some(Instant::now()),
            identity,
        }
    }

    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            error: Some(reason.into()),
            updated: Some(Instant::now()),
            ..Self::default()
        }
    }

    pub fn command(&self, action: Action) -> Result<String, String> {
        if let Some(error) = &self.error {
            return Err(format!("Ganymède indisponible : {error}"));
        }
        if !self.updated.is_some_and(|when| when.elapsed() <= MAX_AGE) {
            return Err("Étape non synchronisée ou trop ancienne".into());
        }
        match action {
            Action::Zaap => self
                .zaap
                .clone()
                .ok_or_else(|| "Zaap recommandé absent ou ambigu".into()),
            Action::Travel => self
                .travel
                .clone()
                .ok_or_else(|| "Destination absente ou ambiguë".into()),
            Action::Combined => {
                let travel = self
                    .travel
                    .as_ref()
                    .ok_or("Destination absente ou ambiguë")?;
                Ok(match &self.zaap {
                    Some(zaap) => format!("{zaap}; {zaap}; {travel}"),
                    None => travel.clone(),
                })
            }
        }
    }
}

pub fn travel_destination(text: &str) -> Option<(i32, i32)> {
    // UI Automation exposes rich text and links as separate nodes/lines.
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    static DESTINATION: OnceLock<Regex> = OnceLock::new();
    static ZAAPI: OnceLock<Regex> = OnceLock::new();
    let destination = DESTINATION.get_or_init(|| Regex::new(
        r"(?i)\b(?:ensuite\s+)?(?:allez|aller|rendez[- ]vous|rejoignez|direction|destination|deplacez[- ]vous|déplacez[- ]vous)\b.{0,42}?\[?\s*(-?\d{1,3})\s*[,;]\s*(-?\d{1,3})\s*\]?"
    ).expect("expression régulière valide"));
    let zaapi =
        ZAAPI.get_or_init(|| Regex::new(r"(?i)\bzaapi?\b").expect("expression régulière valide"));
    let unique: HashSet<(i32, i32)> = destination
        .captures_iter(&joined)
        .filter_map(|captures| {
            // A Zaapi is local transport, never the destination used by /travel.
            if zaapi.is_match(captures.get(0)?.as_str()) {
                return None;
            }
            Some((
                captures.get(1)?.as_str().parse().ok()?,
                captures.get(2)?.as_str().parse().ok()?,
            ))
        })
        .collect();
    if unique.len() == 1 {
        unique.into_iter().next()
    } else if unique.is_empty() {
        // A standalone coordinate label is useful; transport coordinates are not.
        static COORDINATES: OnceLock<Regex> = OnceLock::new();
        let coordinates = COORDINATES.get_or_init(|| {
            Regex::new(r"^\s*\[\s*(-?\d{1,3})\s*[,;]\s*(-?\d{1,3})\s*\]\s*$")
                .expect("expression régulière valide")
        });
        // The coordinate before the progress counter is the player's position,
        // not the destination of the instruction below it.
        static PROGRESS: OnceLock<Regex> = OnceLock::new();
        let progress = PROGRESS.get_or_init(|| {
            Regex::new(r"\b\d{1,5}\s*/\s*\d{1,5}\b").expect("expression régulière valide")
        });
        let destination_text = progress
            .find(text)
            .map_or(text, |counter| &text[counter.end()..]);
        let points: HashSet<(i32, i32)> = destination_text
            .lines()
            .filter_map(|line| {
                let captures = coordinates.captures(line)?;
                Some((captures[1].parse().ok()?, captures[2].parse().ok()?))
            })
            .collect();
        if points.len() == 1 {
            points.into_iter().next()
        } else {
            None
        }
    } else {
        None
    }
}

fn step_identity(text: &str, zaap: Option<&str>, travel: Option<&str>) -> Option<String> {
    static PROGRESS: OnceLock<Regex> = OnceLock::new();
    let progress = PROGRESS.get_or_init(|| {
        Regex::new(r"\b(\d{1,5})\s*/\s*(\d{1,5})\b").expect("expression régulière valide")
    });
    // L'indicateur de progression sert de référence, même si les commandes ne
    // sont pas encore entièrement affichées. Les fractions sans rapport sont
    // écartées en privilégiant le plus grand nombre total d'étapes.
    if let Some((current, total)) = progress
        .captures_iter(text)
        .filter_map(|value| {
            Some((
                value.get(1)?.as_str().parse::<u32>().ok()?,
                value.get(2)?.as_str().parse::<u32>().ok()?,
            ))
        })
        .filter(|(current, total)| *current > 0 && *current <= *total && *total <= 100_000)
        .max_by_key(|(_, total)| *total)
    {
        return Some(format!("step:{current}/{total}"));
    }
    // Include visible step text: two successive instructions may share a route.
    // Without an explicit counter, identical text cannot identify a new step.
    travel.map(|travel| {
        format!(
            "route:{}|{}|{}",
            zaap.unwrap_or(""),
            travel,
            normalize(text)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        )
    })
}

pub fn recommended_zaap<'a>(text: &str, known: &'a [Zaap]) -> Option<&'a Zaap> {
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    static TRANSPORT: OnceLock<Regex> = OnceLock::new();
    let transport = TRANSPORT
        .get_or_init(|| Regex::new(r"(?i)\bzaapi?\b").expect("expression régulière valide"));
    static ENSEVELI: OnceLock<Regex> = OnceLock::new();
    let enseveli = ENSEVELI
        .get_or_init(|| Regex::new(r"\bvillage ensevelli\b").expect("expression régulière valide"));
    let mut matches = HashSet::new();

    for line in joined.lines() {
        let tags: Vec<_> = transport.find_iter(line).collect();
        for (position, tag) in tags.iter().enumerate() {
            if tag.as_str().eq_ignore_ascii_case("zaapi") {
                continue;
            }
            // Stop at the next transport mention on the same line. A later
            // Zaapi name must not become part of the recommended Zaap label.
            let mut end = tags
                .get(position + 1)
                .map_or(line.len(), |next| next.start())
                .min(tag.start() + 100);
            while !line.is_char_boundary(end) {
                end -= 1;
            }
            let label = normalize(&line[tag.start()..end]);
            // Accept the spelling used by some Ganymede instructions while
            // preserving the directory's canonical name and coordinates.
            let label = enseveli.replace_all(&label, "village enseveli");
            let named: Vec<usize> = known
                .iter()
                .enumerate()
                .filter(|(_, zaap)| label.contains(&normalize(&zaap.name)))
                .map(|(index, _)| index)
                .collect();
            if !named.is_empty() {
                matches.extend(named);
                continue;
            }

            // Ganymède peut indiquer « Zaap de Brâkmar » alors que son nom dans
            // la liste est « La Cuirasse ». Une région n'est acceptée que si elle
            // identifie un unique zaap et si son libellé correspond exactement.
            let place = normalize(&line[tag.end()..end]);
            let place = place
                .trim()
                .trim_matches(|c: char| c == ':' || c == '-' || c == '.' || c.is_whitespace());
            let place = [
                "de la ", "de l'", "des ", "du ", "de ", "d'", "a la ", "au ", "a ",
            ]
            .iter()
            .find_map(|prefix| place.strip_prefix(prefix))
            .unwrap_or(place)
            .trim();
            // Certains noms d'usage désignent un zaap précis plutôt qu'une
            // région. Ces correspondances sont exactes et ne dépendent pas du
            // nombre de zaaps ajoutés ultérieurement à cette région.
            let alias = match place
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_end_matches(['.', ',', ';'])
            {
                "astrub" => Some("Cité d'Astrub"),
                "bonta" => Some("Cœur immaculé"),
                "brakmar" => Some("La Cuirasse"),
                _ => None,
            };
            if let Some(name) = alias {
                if let Some((index, _)) = known
                    .iter()
                    .enumerate()
                    .find(|(_, zaap)| normalize(&zaap.name) == normalize(name))
                {
                    matches.insert(index);
                }
                continue;
            }

            let regions: Vec<usize> = known
                .iter()
                .enumerate()
                .filter(|(_, zaap)| normalize(&zaap.region) == place)
                .map(|(index, _)| index)
                .collect();
            if regions.len() == 1 {
                matches.insert(regions[0]);
            }
        }
    }
    if matches.len() == 1 {
        known.get(*matches.iter().next()?)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data;

    #[test]
    fn both_commands_from_same_step() {
        let step = Step::from_text(
            "Zaap de la Cité d’Astrub\nAllez en [4, -19]",
            &data::load().unwrap(),
        );
        assert_eq!(step.command(Action::Zaap).unwrap(), "/zaap 5,-18");
        assert_eq!(step.command(Action::Travel).unwrap(), "/travel 4 -19");
        assert_eq!(
            step.command(Action::Combined).unwrap(),
            "/zaap 5,-18; /zaap 5,-18; /travel 4 -19"
        );
    }

    #[test]
    fn ambiguous_or_missing_data_never_creates_partial_command() {
        let known = data::load().unwrap();
        let step = Step::from_text(
            "Allez en [4,-19]\nAllez en [5,-20]\nZaap de la Cité d'Astrub",
            &known,
        );
        assert!(step.command(Action::Combined).is_err());
        assert!(Step::unavailable("fenêtre fermée")
            .command(Action::Zaap)
            .is_err());
    }

    #[test]
    fn zaapis_do_not_mask_the_recommended_zaap() {
        let known = data::load().unwrap();
        let text = "Zaapi de la Plaine des Scarafeuilles\nZaap de la Cité d'Astrub\nEnsuite allez en [4,-19]";
        let step = Step::from_text(text, &known);
        assert_eq!(
            step.command(Action::Combined).unwrap(),
            "/zaap 5,-18; /zaap 5,-18; /travel 4 -19"
        );
    }

    #[test]
    fn zaapi_before_or_after_zaap_on_same_line_is_ignored() {
        let known = data::load().unwrap();
        for text in [
            "Zaapi de la Plaine des Scarafeuilles ; Zaap de la Cité d'Astrub",
            "Zaap de la Cité d'Astrub ; Zaapi de la Plaine des Scarafeuilles",
        ] {
            assert_eq!(
                recommended_zaap(text, &known).unwrap().command(),
                "/zaap 5,-18"
            );
        }
    }

    #[test]
    fn a_zaapi_alone_never_generates_a_zaap_command() {
        let known = data::load().unwrap();
        let step = Step::from_text(
            "Zaapi de la Cité d'Astrub\nEnsuite allez en [4,-19]",
            &known,
        );
        assert!(step.command(Action::Zaap).is_err());
        assert_eq!(step.command(Action::Combined).unwrap(), "/travel 4 -19");
        assert_eq!(step.command(Action::Travel).unwrap(), "/travel 4 -19");
    }

    #[test]
    fn ignores_a_zaapi_direction_in_travel_destinations() {
        let text = "Allez au Zaapi en [1,2]\nEnsuite allez en [4,-19]";
        assert_eq!(travel_destination(text), Some((4, -19)));
    }

    #[test]
    fn regional_name_matches_only_unique_known_zaap() {
        let known = data::embedded().unwrap();
        let step = Step::from_text(
            "459/774\nEnsuite allez en [-14,40]\nZaap de Brâkmar\nZaapi vers l'Atelier des alchimistes",
            &known,
        );
        assert_eq!(
            step.command(Action::Combined).unwrap(),
            "/zaap -26,37; /zaap -26,37; /travel -14 40"
        );
    }

    #[test]
    fn city_aliases_are_stable_and_ignore_zaapis() {
        let known = data::embedded().unwrap();
        let samples = [
            ("Zaap d'Astrub", "/zaap 5,-18"),
            ("Zaap de Bonta", "/zaap -31,-56"),
            ("Zaap de Brâkmar", "/zaap -26,37"),
        ];
        for (label, expected) in samples {
            let text =
                format!("{label}\nZaapi vers l'Atelier des alchimistes\nEnsuite allez en [4,-19]");
            let step = Step::from_text(&text, &known);
            assert_eq!(step.command(Action::Zaap).unwrap(), expected, "{label}");
            assert_eq!(
                step.command(Action::Combined).unwrap(),
                format!("{expected}; {expected}; /travel 4 -19"),
                "{label}"
            );
        }
    }

    #[test]
    fn astrub_short_name_never_selects_tainela() {
        let known = data::embedded().unwrap();
        let step = Step::from_text("Zaap d'Astrub\nEnsuite allez en [4,-19]", &known);
        assert_eq!(step.command(Action::Zaap).unwrap(), "/zaap 5,-18");
        assert_ne!(step.command(Action::Zaap).unwrap(), "/zaap 1,-32");
    }

    #[test]
    fn step_progress_identifies_same_route_across_distinct_steps() {
        let known = data::embedded().unwrap();
        let a = Step::from_text("458/774\nZaap de Brâkmar\nAllez en [-14,40]", &known);
        let b = Step::from_text("459/774\nZaap de Brâkmar\nAllez en [-14,40]", &known);
        assert_ne!(a.identity, b.identity);
        assert_eq!(a.command(Action::Combined), b.command(Action::Combined));
    }

    #[test]
    fn expired_snapshot_is_rejected() {
        let mut step = Step::from_text(
            "Zaap de la Cité d'Astrub\nAllez en [4,-19]",
            &data::load().unwrap(),
        );
        step.updated = Some(Instant::now() - Duration::from_secs(5));
        assert!(step.command(Action::Combined).is_err());
    }
}

#[cfg(test)]
mod regression_tests {
    use super::*;
    #[test]
    fn destination_matrix_and_dimensions() {
        let known = crate::data::embedded().unwrap();
        for (text, expected) in [
            (
                "1/10\nZaap de Bonta\nAllez en [-30,-55]",
                Some("/zaap -31,-56; /zaap -31,-56; /travel -30 -55"),
            ),
            (
                "2/10\nZaapi de Bonta\nAllez en [-30,-55]",
                Some("/travel -30 -55"),
            ),
            ("3/10\nAllez en [4,-19]", Some("/travel 4 -19")),
            ("4/10\n[4,-19]", Some("/travel 4 -19")),
            ("5/10\nParlez au personnage", None),
            ("6/10\nZaap de Bonta", None),
            ("7/10\nXélorium\nAllez en [4,-19]", None),
            ("8/10\nAllez au Zaapi en [1,2]", None),
            ("9/10\nAllez en [1,2]\nAllez en [3,4]", None),
            ("9/10\n10/10\nZaap de Bonta\nAllez en [1,2]", None),
            ("9/10\nZaap de Bonta\nZaap de Brâkmar\nAllez en [1,2]", None),
        ] {
            assert_eq!(
                Step::from_text(text, &known)
                    .command(Action::Combined)
                    .ok()
                    .as_deref(),
                expected,
                "{text}"
            );
        }
    }
    #[test]
    fn new_snapshot_never_inherits_previous_fields() {
        let known = crate::data::embedded().unwrap();
        let mut state = Step::from_text("1/10\nZaap de Bonta\nAllez en [1,2]", &known);
        assert!(state.zaap.is_some());
        state = Step::from_text("2/10\nAllez en [3,4]", &known);
        assert!(state.zaap.is_none());
        assert_eq!(state.command(Action::Combined).unwrap(), "/travel 3 4");
        state = Step::from_text("3/10\nParlez au personnage", &known);
        assert!(state.zaap.is_none() && state.travel.is_none());
        assert!(state.command(Action::Combined).is_err());
    }
}

#[cfg(test)]
mod fragmented_accessibility_tests {
    use super::*;
    #[test]
    fn pandala_step_with_separate_accessibility_nodes() {
        let text = "[GP9] ALIGNEMENT\n[GP48] NIMOTOPIA\n[-32,40]\n497/774\nParlez à\nPu Shan\npour\nDemander où se procurer un tel masque.\nEnsuite\nallez en\n[24,-27]\nZaap du\nVillage de Pandala\nRessortez\n2 maps en bas\n4 maps à droite";
        let step = Step::from_text(text, &crate::data::embedded().unwrap());
        assert_eq!(
            step.command(Action::Combined).unwrap(),
            "/zaap 20,-29; /zaap 20,-29; /travel 24 -27"
        );
    }

    #[test]
    fn fragmented_links_keep_aliases_and_zaapis_separate() {
        let known = crate::data::embedded().unwrap();
        for (transport, expected) in [
            (
                "Zaap de\nBonta",
                "/zaap -31,-56; /zaap -31,-56; /travel 24 -27",
            ),
            (
                "Zaap d'\nAstrub",
                "/zaap 5,-18; /zaap 5,-18; /travel 24 -27",
            ),
            (
                "Zaap de\nBrâkmar",
                "/zaap -26,37; /zaap -26,37; /travel 24 -27",
            ),
            ("Zaapi du\nVillage de Pandala", "/travel 24 -27"),
        ] {
            let text =
                format!("[-32,40]\n497/774\nEnsuite\nallez en\n[24,-27]\n{transport}\nRessortez");
            assert_eq!(
                Step::from_text(&text, &known)
                    .command(Action::Combined)
                    .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn current_position_is_never_used_when_destination_is_missing() {
        let known = crate::data::embedded().unwrap();
        for instruction in [
            "Parlez au personnage",
            "Zaap du\nVillage de Pandala",
            "Ensuite allez en",
        ] {
            let text = format!("[-32,40]\n497/774\n{instruction}");
            assert!(Step::from_text(&text, &known)
                .command(Action::Combined)
                .is_err());
        }
    }
}

#[cfg(test)]
mod enseveli_tests {
    use super::*;

    #[test]
    fn canonical_and_ganymede_spellings_resolve_to_the_same_zaap() {
        let known = crate::data::embedded().unwrap();
        for name in ["Village enseveli", "Village ensevelli", "VILLAGE ENSEVELLI"] {
            let text = format!("566\n/\n774\nEntrez dans l'Atelier des bûcherons.\nParlez à Enileda.\nEnsuite\nallez en\n[-78,-70]\nZaap\ndu\n{name}\n2 maps en bas\n1 map à gauche\n1 map en bas");
            let step = Step::from_text(&text, &known);
            assert_eq!(
                step.command(Action::Zaap).unwrap(),
                "/zaap -77,-73",
                "{name}"
            );
            assert_eq!(
                step.command(Action::Travel).unwrap(),
                "/travel -78 -70",
                "{name}"
            );
            assert_eq!(
                step.command(Action::Combined).unwrap(),
                "/zaap -77,-73; /zaap -77,-73; /travel -78 -70",
                "{name}"
            );
        }
        assert_eq!(known.len(), 45);
        assert_eq!(
            known
                .iter()
                .filter(|zaap| zaap.name == "Village enseveli")
                .count(),
            1
        );
    }

    #[test]
    fn spelling_alias_does_not_turn_a_zaapi_or_an_unknown_place_into_a_zaap() {
        let known = crate::data::embedded().unwrap();
        let zaapi = Step::from_text(
            "566/774\nZaapi du Village ensevelli\nAllez en [-78,-70]",
            &known,
        );
        assert!(zaapi.command(Action::Zaap).is_err());
        assert_eq!(zaapi.command(Action::Combined).unwrap(), "/travel -78 -70");
        for text in [
            "566/774\nZaap du Village ensevellissement\nAllez en [-78,-70]",
            "566/774\nZaap du Village ensevelli\nZaap de Bonta\nAllez en [-78,-70]",
        ] {
            assert!(Step::from_text(text, &known)
                .command(Action::Combined)
                .is_err());
        }
    }
}
