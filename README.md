# Zaaps Dofus 3

[![Validation Windows](https://github.com/molow159-boop/zaaps-dofus3/actions/workflows/ci.yml/badge.svg)](https://github.com/molow159-boop/zaaps-dofus3/actions/workflows/ci.yml)
[![Version](https://img.shields.io/github/v/release/molow159-boop/zaaps-dofus3)](https://github.com/molow159-boop/zaaps-dofus3/releases/latest)

Utilitaire portable pour **Windows 10/11 x64**, développé en Rust, pour copier les commandes de déplacement à partir des étapes affichées dans **Ganymède**.

## Télécharger

Ouvrir la [dernière version](https://github.com/molow159-boop/zaaps-dofus3/releases/latest), télécharger **Zaaps-Dofus3-1.0.0-Windows-x64.zip**, extraire le dossier puis lancer **Zaaps Dofus 3.exe**.

Aucune installation de Rust n'est nécessaire pour utiliser l'application.

## Fonctionnalités

- Répertoire de **45 zaaps**, recherche immédiate et copie par clic.
- Lecture des étapes de Ganymède via l'accessibilité Windows.
- Trois raccourcis globaux personnalisables.
- Mode automatique avec confirmation de l'étape et protection contre les copies répétées.
- Reconnaissance des noms usuels Astrub, Bonta et Brâkmar.
- Interface sombre responsive, redimensionnement libre et coins arrondis.
- Préférences conservées entre les lancements.

## Utilisation

Ouvrir Ganymède sur le guide souhaité, puis lancer l'application.

| Raccourci par défaut | Commande copiée |
| --- | --- |
| **Ctrl + J** | `/zaap X,Y` du Zaap recommandé |
| **Ctrl + K** | `/travel X Y` vers la destination |
| **Ctrl + L** | Trajet complet disponible |

Avec un Zaap et une destination, le trajet complet prend la forme :

```text
/zaap 20,-29; /zaap 20,-29; /travel 24 -27
```

Avec seulement un Zaapi, ou sans Zaap, seule la commande `/travel X Y` est copiée si la destination est exploitable. Sans destination exploitable, aucune copie n'est effectuée. Les Zaapis ne sont jamais convertis en commandes `/zaap`.

Le bouton **Auto** active la copie à la prochaine nouvelle étape reconnue. L'étape présente à l'activation sert de référence et n'est pas copiée automatiquement. Les raccourcis se modifient dans les réglages.

**L'application copie uniquement dans le presse-papiers.** Elle ne saisit aucune commande dans Dofus et ne joue pas à votre place.

## Préférences et données

Les préférences sont enregistrées dans `%APPDATA%\Zaaps Dofus 3\raccourcis.ini`.

La liste de zaaps est intégrée à l'exécutable. Un fichier `zaaps.csv` placé à côté peut remplacer cette liste ; le modèle se trouve dans `assets/zaaps.csv`.

## Compiler

Prérequis : **Rust 1.99.1**, cible `x86_64-pc-windows-msvc`, Visual Studio Build Tools C++ et Windows SDK. `Cargo.lock` est fourni et doit être conservé.

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --release --locked
cargo build --release --locked
```

Exécutable produit : `target\release\zaaps-dofus3.exe`.

## Limites

La détection dépend du texte accessible exposé par Ganymède. Les étapes incomplètes ou ambiguës sont refusées. Les destinations explicitement dimensionnelles, dont Xélorium, ne produisent pas de trajet si le monde courant ne peut pas être vérifié. Sans compteur, deux étapes dont le texte est identique ne peuvent pas être distinguées.

L'exécutable n'est pas signé numériquement. Les empreintes SHA-256 de la Release permettent de vérifier les fichiers téléchargés. Ne pas désactiver les protections Windows.

## Signaler un problème

Utiliser les [Issues](https://github.com/molow159-boop/zaaps-dofus3/issues) en précisant l'étape, le raccourci concerné et le résultat attendu. Voir [CONTRIBUTING.md](CONTRIBUTING.md) et [SECURITY.md](SECURITY.md).

## Licence et compatibilité

Code source sous [licence MIT](LICENSE). Les ressources graphiques et marques sont décrites dans [NOTICE.md](NOTICE.md).

Projet communautaire indépendant, sans affiliation à Ankama ni à l'équipe Ganymède. DOFUS et Ganymède appartiennent à leurs ayants droit respectifs.
