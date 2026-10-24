# Compiler sous Windows

[Présentation](../README.md) · [Guide d'utilisation](UTILISATION.md) · [Téléchargements](https://github.com/molow159-boop/zaaps-dofus3/releases/latest)

Pour utiliser le logiciel directement, prenez le ZIP **Windows-x64** de la Release. Ce tuto sert à produire votre propre exécutable à partir des sources Rust.

## 1. Installer les outils

- **Windows 10 ou 11 x64**.
- [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio) : sélectionnez **Développement Desktop en C++**, avec les outils MSVC x64 et un Windows SDK.
- [Rust avec rustup](https://rustup.rs/) : utilisez la chaîne Windows MSVC.

Après l'installation, ouvrez une nouvelle fenêtre PowerShell. Le projet fixe Rust **1.99.1** dans `rust-toolchain.toml` ; rustup utilise cette version lorsque vous entrez dans le dossier.

## 2. Récupérer le projet complet

Téléchargez **Zaaps-Dofus3-1.1.4-Sources.zip** depuis la [Release](https://github.com/molow159-boop/zaaps-dofus3/releases/latest) et extrayez-le. Vous pouvez aussi utiliser **Code → Download ZIP** sur la page du dépôt, ou cloner le dépôt avec Git :

```powershell
git clone https://github.com/molow159-boop/zaaps-dofus3.git
cd zaaps-dofus3
```

Ouvrez PowerShell dans le dossier qui contient `Cargo.toml`, `Cargo.lock`, `build.rs`, `src` et `assets`. Conservez les ressources graphiques et le fichier `Cargo.lock` fournis.

## 3. Installer la chaîne et compiler

```powershell
rustup toolchain install 1.99.1 --profile minimal --component rustfmt --component clippy
rustup target add x86_64-pc-windows-msvc --toolchain 1.99.1
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --release --locked
cargo build --release --locked
```

Le premier lancement télécharge les dépendances et peut prendre plusieurs minutes. Les compilations suivantes réutilisent le cache local.

L'exécutable se trouve ensuite dans :

```text
target\release\zaaps-dofus3.exe
```

Vous pouvez le copier dans un autre dossier et le renommer **Zaaps Dofus 3.exe**. Les ressources et les 45 Zaaps sont intégrés. Rust n'est pas nécessaire sur le poste qui utilisera l'exécutable.

## Si la compilation s'arrête

| Message ou situation | À vérifier |
| --- | --- |
| `cargo` ou `rustup` introuvable | Rouvrez PowerShell après l'installation de Rust. |
| `link.exe` ou Windows SDK introuvable | Ajoutez les outils MSVC x64 et le SDK dans Visual Studio Build Tools. |
| Ressource `assets/...` manquante | Extrayez l'archive complète ; ne copiez pas seulement le dossier `src`. |
| Téléchargement des dépendances impossible | Vérifiez la connexion Internet et relancez la même commande. |
| Fichier EXE impossible à remplacer | Fermez l'instance de Zaaps qui utilise cet exécutable. |

Les essais Windows natifs, séparés de la suite automatique, sont décrits dans [CONTRIBUTING.md](../CONTRIBUTING.md). Ils nécessitent un poste de test et peuvent remplacer le contenu du presse-papiers.
