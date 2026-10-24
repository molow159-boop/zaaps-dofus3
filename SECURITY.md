# Sécurité

## Versions prises en charge

La version 1.1.0 est la version actuelle du projet.

## Fonctionnement

- Lecture de Ganymède par l'accessibilité Windows.
- Raccourcis globaux enregistrés avec les API Windows.
- Copie des commandes dans le presse-papiers, sans exécution dans Dofus.
- Préférences conservées localement dans `%APPDATA%\Zaaps Dofus 3\raccourcis.ini`.
- Aucune connexion utilisateur ni mise à jour automatique intégrée.
- Vérification facultative des nouvelles Releases stables par HTTPS vers l'API publique de GitHub, sans identifiant ni jeton ; aucun téléchargement ni installation automatique.
- Démarrage Windows facultatif via une valeur propre au logiciel dans la clé Run de l'utilisateur courant.
- Surveillance du presse-papiers après une copie du logiciel, sans conservation du contenu copié par les autres applications.
- Diagnostic local avec aperçu avant copie ; aucune transmission automatique.

Les fichiers `SHA256.txt` des Releases permettent de vérifier l'intégrité des téléchargements. L'exécutable n'est pas signé Authenticode.

## Signaler un problème

Pour un bogue fonctionnel, ouvrir une Issue avec les étapes de reproduction. Ne pas publier de mot de passe, de jeton ou de donnée personnelle.

Pour une vulnérabilité, privilégier un signalement privé depuis l'onglet Security du dépôt lorsqu'il est disponible.
