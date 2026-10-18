use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Zaap {
    pub name: String,
    pub region: String,
    pub x: i32,
    pub y: i32,
}

impl Zaap {
    pub fn command(&self) -> String {
        format!("/zaap {},{}", self.x, self.y)
    }
}

pub fn normalize(input: &str) -> String {
    input
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' => 'a',
            'ç' => 'c',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' | 'ì' | 'í' => 'i',
            'ô' | 'ö' | 'ò' | 'ó' => 'o',
            'ù' | 'û' | 'ü' | 'ú' => 'u',
            'ÿ' => 'y',
            'œ' => 'o',
            '’' => '\'',
            _ => c,
        })
        .collect()
}

pub fn load() -> Result<Vec<Zaap>, String> {
    let external = std::env::current_exe()
        .ok()
        .and_then(|file| file.parent().map(|folder| folder.join("zaaps.csv")));
    match external {
        Some(path) if path.exists() => {
            let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            parse_csv(&content)
        }
        _ => embedded(),
    }
}

pub fn embedded() -> Result<Vec<Zaap>, String> {
    parse_csv(include_str!("../assets/zaaps.csv"))
}

fn parse_csv(csv: &str) -> Result<Vec<Zaap>, String> {
    if csv
        .trim_start_matches('\u{feff}')
        .lines()
        .next()
        .map(str::trim)
        != Some("nom;x;y;region;source")
    {
        return Err("En-tête CSV invalide (nom;x;y;region;source)".into());
    }
    let mut result = Vec::new();
    let mut ids = HashSet::new();
    for (index, line) in csv.lines().enumerate().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<_> = line.trim_end_matches('\r').split(';').collect();
        if fields.len() != 5 {
            return Err(format!("Données invalides à la ligne {}", index + 1));
        }
        let x = fields[1]
            .parse::<i32>()
            .map_err(|_| format!("Coordonnée X invalide : {}", index + 1))?;
        let y = fields[2]
            .parse::<i32>()
            .map_err(|_| format!("Coordonnée Y invalide : {}", index + 1))?;
        if fields[0].trim().is_empty()
            || fields[3].trim().is_empty()
            || fields[0].chars().count() > 100
            || fields[3].chars().count() > 100
            || !(-1000..=1000).contains(&x)
            || !(-1000..=1000).contains(&y)
        {
            return Err(format!("Valeur invalide à la ligne {}", index + 1));
        }
        let key = normalize(fields[0]);
        if !ids.insert(key) {
            return Err(format!("Zaap en double : {}", fields[0]));
        }
        result.push(Zaap {
            name: fields[0].into(),
            region: fields[3].into(),
            x,
            y,
        });
    }
    result.sort_by_key(|z| normalize(&z.name));
    if result.is_empty() || result.len() > 500 {
        return Err("La liste doit contenir de 1 à 500 zaaps".into());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sorted_directory_is_complete() {
        let all = embedded().unwrap();
        assert_eq!(all.len(), 45);
        assert!(all
            .windows(2)
            .all(|pair| normalize(&pair[0].name) <= normalize(&pair[1].name)));
    }
    #[test]
    fn astrub_has_correct_coordinates() {
        let all = embedded().unwrap();
        assert_eq!(
            all.iter()
                .find(|z| z.name == "Cité d'Astrub")
                .unwrap()
                .command(),
            "/zaap 5,-18"
        );
    }
}
