<p align="center">
  <img src="assets/icon.png" alt="Icône de Zaaps Dofus 3" width="72">
</p>

<h1 align="center">Zaaps Dofus 3</h1>

<p align="center"><strong>Vos trajets Ganymède à portée de raccourci.</strong></p>

<p align="center">
  <a href="https://github.com/molow159-boop/zaaps-dofus3/releases/latest"><img src="https://img.shields.io/github/v/release/molow159-boop/zaaps-dofus3?label=Version&amp;color=82bc35" alt="Dernière version"></a>
  <img src="https://img.shields.io/badge/Windows-10%20%2F%2011-0078D4" alt="Windows 10 et 11">
  <a href="https://github.com/molow159-boop/zaaps-dofus3/actions/workflows/ci.yml"><img src="https://github.com/molow159-boop/zaaps-dofus3/actions/workflows/ci.yml/badge.svg" alt="Validation Windows"></a>
</p>

<p align="center">
  <a href="https://github.com/molow159-boop/zaaps-dofus3/releases/latest"><strong>Télécharger pour Windows</strong></a>
  &nbsp; · &nbsp;
  <a href="#prise-en-main">Prise en main</a>
  &nbsp; · &nbsp;
  <a href="https://github.com/molow159-boop/zaaps-dofus3/issues">Signaler un problème</a>
</p>

Retrouvez un zaap, préparez votre déplacement et copiez votre trajet sans ressaisir les coordonnées. **Zaaps Dofus 3** est un utilitaire portable en Rust pour **Windows 10/11 x64**, conçu pour accompagner les étapes affichées dans **Ganymède**.

![Interface de Zaaps Dofus 3 : liste des zaaps et mode automatique activé](assets/screenshots/interface.jpg)

## L'essentiel, dans une seule fenêtre

| Fonctionnalité | Au quotidien |
| --- | --- |
| **45 zaaps intégrés** | Parcourez la liste et copiez une commande en un clic. |
| **Recherche immédiate** | Retrouvez un zaap par son nom ou sa région. |
| **Trajets Ganymède** | Récupérez le Zaap recommandé et les coordonnées de destination. |
| **Trois raccourcis configurables** | Choisissez vos touches pour le Zaap, le voyage et le trajet complet. |
| **Mode automatique** | Copiez une fois le trajet à chaque nouvelle étape reconnue. |
| **Interface adaptable** | Redimensionnez librement la fenêtre ; les colonnes s'ajustent à sa largeur. |

## En images

| Recherche rapide | Raccourcis personnalisables |
| :---: | :---: |
| [![Recherche des zaaps liés à Astrub](assets/screenshots/recherche.jpg)](assets/screenshots/recherche.jpg) | [![Réglages des trois raccourcis Ganymède](assets/screenshots/raccourcis.jpg)](assets/screenshots/raccourcis.jpg) |
| Un nom ou une région suffit pour filtrer la liste. | Ctrl+J, Ctrl+K et Ctrl+L par défaut, modifiables dans les réglages. |

## Prise en main

1. Dans la [dernière Release](https://github.com/molow159-boop/zaaps-dofus3/releases/latest), téléchargez **Zaaps-Dofus3-1.0.2-Windows-x64.zip** et extrayez entièrement l'archive.
2. Ouvrez Ganymède sur votre guide, puis lancez **Zaaps Dofus 3.exe**.
3. Cliquez sur un zaap ou utilisez un raccourci, puis collez la commande dans Dofus.

**Aucune installation de Rust n'est nécessaire.** Le logiciel copie les commandes dans le presse-papiers ; vous gardez la main sur leur utilisation dans Dofus.

## Vos trois raccourcis

| Raccourci par défaut | Action |
| --- | --- |
| **Ctrl + J** | Copier le Zaap recommandé : `/zaap X,Y`. |
| **Ctrl + K** | Copier la destination : `/travel X Y`. |
| **Ctrl + L** | Copier le trajet complet disponible. |

Ouvrez les réglages avec l'icône d'engrenage pour personnaliser les touches. Vos préférences sont conservées entre les lancements.

## Le bon trajet selon l'étape

Avec un Zaap et une destination, le trajet complet contient deux commandes `/zaap`, puis le voyage :

```text
/zaap 20,-29; /zaap 20,-29; /travel 24 -27
```

| Indications disponibles dans Ganymède | Résultat du trajet complet |
| --- | --- |
| Zaap réel et destination exploitable | Double `/zaap`, puis `/travel`. |
| Zaapi seul et destination exploitable | `/travel` uniquement ; le Zaapi est ignoré. |
| Destination exploitable sans Zaap | `/travel` uniquement. |
| Aucune destination exploitable | Aucune copie. |

Les noms usuels **Astrub, Bonta et Brâkmar** sont reconnus.

## Le mode automatique

Activez **Auto** pour copier le trajet à la prochaine nouvelle étape reconnue de Ganymède. Les données sont confirmées avant la copie et une même étape ne déclenche pas de copies répétées.

L'étape présente à l'activation sert de référence : elle n'est pas copiée automatiquement. Les raccourcis restent disponibles pour demander une commande manuellement.

<details>
<summary><strong>Données et compatibilité</strong></summary>

La liste des zaaps est intégrée à l'exécutable. Un fichier `zaaps.csv` placé à côté peut remplacer cette liste ; le modèle se trouve dans `assets/zaaps.csv`.

Les préférences sont enregistrées dans `%APPDATA%\Zaaps Dofus 3\raccourcis.ini`.

La détection dépend du texte accessible exposé par Ganymède. Les étapes incomplètes ou ambiguës sont refusées. Les destinations explicitement dimensionnelles, dont Xélorium, ne produisent pas de trajet si le monde courant ne peut pas être vérifié. Sans compteur, deux étapes dont le texte est identique ne peuvent pas être distinguées.

L'exécutable n'est pas signé numériquement. Le fichier `SHA256.txt` de la Release permet de vérifier l'intégrité des téléchargements.

</details>

<details>
<summary><strong>Compiler depuis les sources</strong></summary>

Prérequis : **Rust 1.99.1**, cible `x86_64-pc-windows-msvc`, Visual Studio Build Tools C++ et Windows SDK. Conservez le fichier `Cargo.lock` fourni.

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --release --locked
cargo build --release --locked
```

Exécutable produit : `target\release\zaaps-dofus3.exe`.

</details>

## Participer

Un problème de lecture ou une suggestion ? Ouvrez une [Issue](https://github.com/molow159-boop/zaaps-dofus3/issues) avec l'étape concernée et le résultat attendu. Pour contribuer, consultez [CONTRIBUTING.md](CONTRIBUTING.md). Les informations de sécurité figurent dans [SECURITY.md](SECURITY.md).

---

Code source sous [licence MIT](LICENSE). Ressources et marques : [NOTICE.md](NOTICE.md).

Projet communautaire indépendant, sans affiliation à Ankama ni à l'équipe Ganymède. DOFUS et Ganymède appartiennent à leurs ayants droit respectifs.
