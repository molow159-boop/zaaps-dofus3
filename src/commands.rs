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
    pub fn progress(&self) -> Option<&str> {
        let label = self.identity.as_deref()?.strip_prefix("step:")?;
        let (current, total) = label.split_once('/')?;
        let current = current.parse::<u32>().ok()?;
        let total = total.parse::<u32>().ok()?;
        (current > 0 && current <= total).then_some(label)
    }

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
        let next = next_route(text);
        if let Some(route) = next {
            let dimensions = next_dimensions(route);
            if dimensions.len() == 1 {
                // Divine-dimension coordinates belong to another world. Prepare
                // access to the tower instead, using the configured Zaap list.
                let mut tower_zaaps = known.iter().filter(|z| {
                    normalize(&z.name) == "routes rocailleuses" && z.x == -20 && z.y == -20
                });
                let tower_zaap = tower_zaaps.next();
                if let Some(zaap) = tower_zaap.filter(|_| tower_zaaps.next().is_none()) {
                    let zaap = Some(zaap.command());
                    let travel = Some("/travel -22 -24".to_owned());
                    return Self {
                        identity: step_identity(text, zaap.as_deref(), travel.as_deref()),
                        zaap,
                        travel,
                        error: None,
                        updated: Some(Instant::now()),
                    };
                }
                return Self {
                    identity: step_identity(text, None, None),
                    ..Self::unavailable("Zaap d'accès à la Tour absent ou ambigu")
                };
            }
        }
        // A dimensional reference in the current action must not hide an
        // explicit next world route. Both its transport and target must come
        // from that next section, never from the preceding action.
        let route = if mentions_dimension(text) {
            match next.filter(|route| {
                !mentions_dimension(route)
                    && recommended_zaap(route, known).is_some()
                    && travel_destination(route).is_some()
            }) {
                Some(route) => route,
                None => {
                    return Self {
                        identity: step_identity(text, None, None),
                        ..Self::unavailable("Destination dimensionnelle : trajet non vérifiable")
                    };
                }
            }
        } else {
            text
        };
        let zaap = recommended_zaap(route, known).map(|z| z.command());
        static ZAAP_MENTION: OnceLock<Regex> = OnceLock::new();
        if zaap.is_none()
            && ZAAP_MENTION
                .get_or_init(|| Regex::new(r"(?i)\bzaap\b").expect("expression régulière valide"))
                .is_match(route)
        {
            return Self {
                identity: step_identity(text, None, None),
                ..Self::unavailable("Zaap recommandé absent ou ambigu")
            };
        }
        let travel = travel_destination(route).map(|(x, y)| format!("/travel {} {}", x, y));
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

fn next_route(text: &str) -> Option<&str> {
    static NEXT: OnceLock<Regex> = OnceLock::new();
    let next = NEXT.get_or_init(|| {
        Regex::new(r"(?i)\bensuite\s+(?:allez|aller|rendez[- ]vous|rejoignez|direction|destination|deplacez[- ]vous|déplacez[- ]vous)\b")
            .expect("expression régulière valide")
    });
    let mut matches = next.find_iter(text);
    let start = matches.next()?.start();
    matches.next().is_none().then_some(&text[start..])
}

fn next_dimensions(route: &str) -> HashSet<String> {
    static DIMENSIONS: OnceLock<Regex> = OnceLock::new();
    let dimensions = DIMENSIONS.get_or_init(|| {
        Regex::new(r"\b(?:enutrosor|srambad|xelorium|ecaflipus|osavora)\b")
            .expect("expression régulière valide")
    });
    // Join fragmented accessibility nodes, but stop at coordinates or sentence
    // boundaries so a later instruction cannot redefine the destination.
    let joined = normalize(&route.split_whitespace().collect::<Vec<_>>().join(" "));
    let heading = joined.split(['[', '.', ';', '!', '?']).next().unwrap_or("");
    dimensions
        .find_iter(heading)
        .map(|name| name.as_str().to_owned())
        .collect()
}

fn mentions_dimension(text: &str) -> bool {
    let normalized = normalize(text);
    [
        "xelorium",
        "dimension",
        "enutrosor",
        "srambad",
        "ecaflipus",
        "osavora",
    ]
    .iter()
    .any(|name| normalized.contains(name))
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
    let names: Vec<String> = known.iter().map(|zaap| normalize(&zaap.name)).collect();
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
            let mut occurrences = Vec::new();
            for (index, name) in names.iter().enumerate() {
                for (start, _) in label.match_indices(name.as_str()) {
                    let end = start + name.len();
                    let left_boundary = label[..start]
                        .chars()
                        .next_back()
                        .is_none_or(|c| !c.is_alphanumeric());
                    let right_boundary = label[end..]
                        .chars()
                        .next()
                        .is_none_or(|c| !c.is_alphanumeric());
                    if left_boundary && right_boundary {
                        occurrences.push((index, start, end));
                    }
                }
            }
            // A name nested in a longer match is the same mention, e.g.
            // "Cimetière" inside "Cimetière primitif". Separate mentions
            // remain separate candidates and must still be rejected as ambiguous.
            let named: Vec<usize> = occurrences
                .iter()
                .filter(|(_, start, end)| {
                    !occurrences.iter().any(|(_, outer_start, outer_end)| {
                        outer_start <= start
                            && end <= outer_end
                            && (outer_start < start || end < outer_end)
                    })
                })
                .map(|(index, _, _)| *index)
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
mod dimension_context_tests {
    use super::*;

    const STEP: &str = "[6,13]\n643/774\nEntrez dans la faille.\nPrenez le portail.\nUne fois dans la Dimension Obscure, combattez un Panterreur pour obtenir un Panterreur en personnage suiveur.\nEnsuite allez en [7,20]\nZaap du Rivage sufokien\n3 maps à gauche\n2 maps en haut";

    #[test]
    fn each_named_next_dimension_routes_to_the_tower() {
        let known = crate::data::embedded().unwrap();
        for name in [
            "Enutrosor",
            "Srambad",
            "Xelorium",
            "Xélorium",
            "Ecaflipus",
            "Écaflipus",
            "la dimension Osavora",
            "SRAMBAD",
        ] {
            for suffix in [" en [8,3]", ""] {
                let text = format!("[-34,-58]\n630/774\nParlez à Larry Poller.\nVérifiez que vous avez les ressources suivantes :\n1 Venin d'Éperfide\n1 Pince de Lucrane\nEnsuite\nallez en\n{name}{suffix}");
                let step = Step::from_text(&text, &known);
                assert_eq!(step.identity.as_deref(), Some("step:630/774"));
                assert_eq!(
                    step.command(Action::Zaap).unwrap(),
                    "/zaap -20,-20",
                    "{text}"
                );
                assert_eq!(
                    step.command(Action::Travel).unwrap(),
                    "/travel -22 -24",
                    "{text}"
                );
                assert_eq!(
                    step.command(Action::Combined).unwrap(),
                    "/zaap -20,-20; /zaap -20,-20; /travel -22 -24",
                    "{text}"
                );
            }
        }
    }

    #[test]
    fn tower_route_never_reuses_a_previous_route() {
        let known = crate::data::embedded().unwrap();
        let first = Step::from_text("629/774\nEnsuite allez en [1,2]\nZaap de Bonta", &known);
        assert!(first.command(Action::Combined).is_ok());
        let tower = Step::from_text("630/774\nEnsuite allez en Srambad en [8,3]", &known);
        assert_eq!(
            tower.command(Action::Combined).unwrap(),
            "/zaap -20,-20; /zaap -20,-20; /travel -22 -24"
        );
        let empty = Step::from_text("631/774\nParlez au personnage de Srambad.", &known);
        assert!(empty.zaap.is_none() && empty.travel.is_none());
        assert!(empty.command(Action::Combined).is_err());
    }

    #[test]
    fn tower_access_requires_an_unambiguous_known_zaap() {
        let known = crate::data::embedded().unwrap();
        let mut without_access = known.clone();
        without_access.retain(|z| z.name != "Routes Rocailleuses");
        let mut duplicate = known.clone();
        duplicate.push(
            known
                .iter()
                .find(|z| z.name == "Routes Rocailleuses")
                .unwrap()
                .clone(),
        );
        for directory in [without_access, duplicate] {
            let step = Step::from_text("630/774\nEnsuite allez en Srambad en [8,3]", &directory);
            assert!(step.command(Action::Combined).is_err());
        }
    }

    #[test]
    fn dimension_in_current_action_does_not_block_the_next_world_route() {
        let known = crate::data::embedded().unwrap();
        for text in [
            STEP.to_owned(),
            STEP.replace("Ensuite allez en", "Ensuite\nallez en")
                .replace("Zaap du Rivage sufokien", "Zaap\ndu\nRivage sufokien"),
        ] {
            let step = Step::from_text(&text, &known);
            assert_eq!(step.identity.as_deref(), Some("step:643/774"));
            assert_eq!(step.command(Action::Zaap).unwrap(), "/zaap 10,22");
            assert_eq!(step.command(Action::Travel).unwrap(), "/travel 7 20");
            assert_eq!(
                step.command(Action::Combined).unwrap(),
                "/zaap 10,22; /zaap 10,22; /travel 7 20"
            );
        }
    }

    #[test]
    fn only_the_next_route_supplies_coordinates_and_transport() {
        let text = format!("Allez en [1,2]\nZaap de Bonta\n{STEP}");
        let step = Step::from_text(&text, &crate::data::embedded().unwrap());
        assert_eq!(
            step.command(Action::Combined).unwrap(),
            "/zaap 10,22; /zaap 10,22; /travel 7 20"
        );
    }

    #[test]
    fn dimensional_or_unconfirmed_next_routes_remain_blocked() {
        let known = crate::data::embedded().unwrap();
        for text in [
            "643/774\nXélorium\nAllez en [7,20]",
            "643/774\nDimension Obscure\nEnsuite allez en [7,20]",
            "643/774\nDimension Obscure\nEnsuite allez en [7,20]\nZaapi du Rivage sufokien",
            "643/774\nZaap du Rivage sufokien\nDimension Obscure\nEnsuite allez en [7,20]",
            "643/774\nDimension Obscure\nEnsuite allez en [7,20] dans Xélorium\nZaap du Rivage sufokien",
            "643/774\nDimension Obscure\nEnsuite allez en [7,20]\nZaap du Rivage sufokien\nZaap de Bonta",
            "643/774\nDimension Obscure\nEnsuite allez en [7,20]\nAllez en [1,2]\nZaap du Rivage sufokien",
            "643/774\nDimension Obscure\nEnsuite allez en [7,20]\nEnsuite allez en [7,20]\nZaap du Rivage sufokien",
            "643/774\nEnsuite allez en Srambad ou Enutrosor en [8,3]",
            "643/774\nEnsuite allez en Srambad en [8,3]\nEnsuite allez en Xélorium en [1,2]",
        ] {
            let step = Step::from_text(text, &known);
            for action in [Action::Zaap, Action::Travel, Action::Combined] {
                assert!(step.command(action).is_err(), "{text}");
            }
        }
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

#[cfg(test)]
mod cemetery_tests {
    use super::*;

    #[test]
    fn primitive_cemetery_step_is_not_confused_with_incarnam_cemetery() {
        let known = crate::data::embedded().unwrap();
        let text = "[4,12]\n587\n/\n774\nEntrez dans la maison.\nParlez à Karine.\nEnsuite parlez à Tati le Collectionneur.\nEnsuite\nallez en\n[-15,19]\nZaap\ndu\nCimetière primitif\n1 map à gauche\nEntrez dans la grotte\nRessortez par la gauche\n1 map à gauche";
        let step = Step::from_text(text, &known);
        assert_eq!(step.command(Action::Zaap).unwrap(), "/zaap -12,19");
        assert_eq!(step.command(Action::Travel).unwrap(), "/travel -15 19");
        assert_eq!(
            step.command(Action::Combined).unwrap(),
            "/zaap -12,19; /zaap -12,19; /travel -15 19"
        );
    }

    #[test]
    fn standalone_cemetery_and_primitive_cemetery_are_order_independent() {
        let mut known = crate::data::embedded().unwrap();
        for _ in 0..2 {
            for (name, command) in [
                ("Cimetière", "/zaap 3,0"),
                ("Cimetière primitif", "/zaap -12,19"),
            ] {
                let step = Step::from_text(&format!("Zaap du {name}\nAllez en [-15,19]"), &known);
                assert_eq!(step.command(Action::Zaap).unwrap(), command);
            }
            known.reverse();
        }
    }

    #[test]
    fn distinct_cemetery_mentions_remain_ambiguous() {
        let known = crate::data::embedded().unwrap();
        for text in [
            "Zaap du Cimetière primitif ; Zaap du Cimetière\nAllez en [-15,19]",
            "Zaap du Cimetière primitif puis Cimetière\nAllez en [-15,19]",
            "Zaap du Cimetière puis Cimetière primitif\nAllez en [-15,19]",
            "Zaap du Supercimetière primitif\nAllez en [-15,19]",
        ] {
            assert!(
                Step::from_text(text, &known)
                    .command(Action::Combined)
                    .is_err(),
                "{text}"
            );
        }
        let step = Step::from_text("Zaapi du Cimetière primitif\nAllez en [-15,19]", &known);
        assert!(step.command(Action::Zaap).is_err());
        assert_eq!(step.command(Action::Combined).unwrap(), "/travel -15 19");
    }
}
