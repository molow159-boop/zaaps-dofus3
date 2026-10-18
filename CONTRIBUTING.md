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

Les changements d'illustrations doivent être accompagnés d'informations sur leur provenance et leurs droits de redistribution. Toute nouvelle dépendance doit être examinée et ajoutée au verrouillage `Cargo.lock`.
