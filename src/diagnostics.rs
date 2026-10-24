use crate::{
    commands::{Action, Step},
    settings::Shortcuts,
};

pub fn summary(step: &Step, settings: &Shortcuts, tray: bool, replaced: bool) -> String {
    let yes = |value| if value { "oui" } else { "non" };
    let route = step.command(Action::Combined).unwrap_or_else(|error| error);
    format!("Zaaps Dofus 3 {}\nSystème : Windows x64\nÉtape : {}\nAuto : {}\nMode compact : {}\nIcône près de l'horloge : {}\nCommande remplacée : {}\nZaap : {}\nVoyage : {}\nTrajet / état : {}\n",
        env!("CARGO_PKG_VERSION"), step.progress().unwrap_or("non reconnue"), if settings.automatic { "actif" } else { "suspendu" },
        yes(settings.compact), yes(tray), yes(replaced),
        step.command(Action::Zaap).ok().as_deref().unwrap_or("indisponible"),
        step.command(Action::Travel).ok().as_deref().unwrap_or("indisponible"), route)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostic_reports_the_route_without_dumping_the_guide_or_clipboard() {
        let step = Step::from_text(
            "[GP9] NomPersonnel\n630/774\nParlez au PNJ privé.\nEnsuite allez en Srambad en [8,3]",
            &crate::data::embedded().unwrap(),
        );
        let report = summary(&step, &Shortcuts::default(), true, true);
        assert!(report.contains("630/774") && report.contains("/travel -22 -24"));
        assert!(!report.contains("NomPersonnel") && !report.contains("PNJ privé"));
        let mut without_counter = step;
        without_counter.identity = Some("text:nom-personnel".into());
        assert!(without_counter.progress().is_none());
        assert!(
            !summary(&without_counter, &Shortcuts::default(), false, false)
                .contains("nom-personnel")
        );
    }
}
