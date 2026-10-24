# Contribuer

Merci de conserver un code clair, des changements ciblés et des explications techniques simples.

Avant une pull request, exécuter :

```powershell
cargo fmt --all -- --check
cargo test --release --locked
cargo clippy --all-targets --locked -- -D warnings
cargo build --release --locked
```

Signaler les régressions de lecture Ganymède, le mode automatique, les Zaapis, les raccourcis et le redimensionnement. Ajouter un test de non-régression quand un bug est corrigé. Ne pas joindre de données personnelles dans les logs ou captures.

Les essais Windows natifs sont séparés des tests automatiques : ils utilisent un bureau interactif, une fenêtre temporaire, le presse-papiers réel et une connexion HTTPS à GitHub. Pour les lancer sur un poste de test :

```powershell
cargo test --release --locked native_ -- --ignored --test-threads=1
```

Ces essais remplacent le contenu du presse-papiers. Ils ne modifient pas le démarrage avec Windows.

Les changements d'illustrations doivent être accompagnés d'informations sur leur provenance et leurs droits de redistribution. Toute nouvelle dépendance doit être examinée et ajoutée au verrouillage `Cargo.lock`.
