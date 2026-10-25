# Utiliser Zaaps Dofus 3

[Téléchargements Windows](https://github.com/molow159-boop/zaaps-dofus3/releases/latest) · [Présentation](../README.md) · [Compiler soi-même](COMPILATION.md)

## Premier lancement

1. Téléchargez **Zaaps-Dofus3-1.1.5-Windows-x64.zip** et extrayez entièrement l'archive dans le dossier de votre choix.
2. Ouvrez Ganymède sur votre guide.
3. Lancez **Zaaps Dofus 3.exe**. Rust n'est pas nécessaire pour utiliser cet exécutable.
4. Vérifiez le numéro d'étape, le Zaap et la destination affichés dans le compagnon.
5. Cliquez sur un bouton de copie, puis collez la commande dans Dofus.

![Le compagnon compact et ses trois boutons de copie](../assets/screenshots/compact.jpg)

Les commandes sont copiées dans le presse-papiers. Leur utilisation dans Dofus reste manuelle.

## Copier le trajet de l'étape

| Bouton | Raccourci par défaut | Résultat |
| --- | --- | --- |
| **Zaap** | **Ctrl+J** | `/zaap X,Y` pour le Zaap recommandé. |
| **Travel** | **Ctrl+K** | `/travel X Y` vers la destination reconnue. |
| **Trajet complet** | **Ctrl+L** | Double `/zaap`, puis `/travel` lorsqu'un Zaap réel et une destination sont disponibles. |

Avec un Zaapi seul, ou des coordonnées sans Zaap, le trajet complet contient uniquement le `/travel` exploitable. Sans destination utilisable, aucune commande n'est copiée. Un bouton grisé indique que les données nécessaires ne sont pas disponibles.

Pour une prochaine destination annoncée en Enutrosor, Srambad, Xélorium, Ecaflipus ou Osavora, le trajet prépare l'accès à la Tour des Voyageurs Dimensionnels :

```text
/zaap -20,-20; /zaap -20,-20; /travel -22 -24
```

## Activer la copie automatique

Cliquez sur **Auto** ou utilisez **Ctrl+Shift+A**. Lorsque le bouton indique **Auto : ON**, une nouvelle étape reconnue et confirmée déclenche une seule copie du trajet disponible.

L'étape présente à l'activation sert de référence : elle n'est pas copiée immédiatement. Vous pouvez toujours demander sa commande avec les boutons de copie. Une étape sans destination exploitable ne produit aucune copie.

## Accrocher le compagnon à Ganymède

1. Ouvrez les réglages avec l'engrenage.
2. Cochez **Accrocher à Ganymède**.
3. Choisissez **En haut**, **En bas**, **À gauche** ou **À droite**, puis cliquez sur **Enregistrer**.

Déplacer l'une des fenêtres entraîne l'autre. Les bords visibles restent accolés. En haut et en bas, Zaaps suit la largeur de Ganymède ; redimensionnez Ganymède pour ajuster cette largeur et le bord supérieur ou inférieur de Zaaps pour sa hauteur. Sur les côtés, les fenêtres s'alignent en haut et le compagnon conserve sa taille compacte.

La vue accrochée démarre à environ 225 pixels de haut. **Afficher Zaaps** ouvre le gestionnaire à part. Les réglages et le diagnostic s'ouvrent également dans leurs propres fenêtres.

Pour séparer les fenêtres, décochez l'accrochage dans les réglages et enregistrez. Le côté et l'option sont mémorisés. La taille de Ganymède n'est pas modifiée par le logiciel.

L'accrochage se met en pause si une fenêtre est masquée, réduite ou maximisée. S'il manque de la place à l'écran, choisissez un autre côté ou réduisez les fenêtres. Si Ganymède est trop étroit pour les commandes, élargissez-le.

## Retrouver un Zaap dans la liste

Cliquez sur **Afficher Zaaps** dans le compagnon accroché. Sans accrochage, **Afficher les 45 Zaaps** déplie la liste dans la fenêtre principale ; **Revenir au mode compact** la replie.

![Le gestionnaire indépendant des 45 Zaaps](../assets/screenshots/interface.jpg)

La recherche accepte un nom, une région ou des coordonnées. Cliquer sur une ligne ou son icône de copie prépare la commande de ce Zaap. Le nombre de colonnes s'adapte à la largeur de la fenêtre.

![Recherche par nom ou région : Astrub](../assets/screenshots/recherche.jpg)

Fermer le gestionnaire indépendant laisse le compagnon, les raccourcis et Auto actifs.

## Personnaliser les touches et les confirmations

Dans les réglages, cliquez sur une action puis appuyez sur la combinaison souhaitée. Choisissez cinq raccourcis différents, puis **Enregistrer**. **Annuler** conserve les réglages précédents.

![Exemple de raccourcis personnalisés et choix de l'accrochage](../assets/screenshots/reglages-raccourcis.jpg)

Les deux raccourcis supplémentaires sont **Ctrl+Shift+A** pour Auto et **Ctrl+Shift+Z** pour afficher ou masquer la fenêtre. Pendant la configuration, les raccourcis et la copie automatique sont suspendus jusqu'à la fermeture des réglages.

![Options de confirmation et de démarrage](../assets/screenshots/reglages-confort.jpg)

Choisissez une confirmation **visuelle**, un **son léger** ou le **silence**. Les notifications visuelles suivent les paramètres de Windows.

La surveillance du presse-papiers signale lorsqu'une autre copie remplace la commande. Le logiciel ne restaure jamais une ancienne commande automatiquement : utilisez **Trajet complet** dans la vue accrochée, ou **Recopier le trajet courant** dans la vue compacte normale, pour demander les données de l'étape actuelle.

## Réduire près de l'horloge

Le bouton **−** masque le compagnon près de l'horloge, en gardant les raccourcis et Auto actifs. Cliquez sur l'icône pour le retrouver, ou utilisez **Ctrl+Shift+Z**. Le clic droit propose l'affichage, Auto, la copie du trajet courant et **Quitter**.

Le bouton **×** de la fenêtre principale quitte complètement le logiciel. Si l'icône de notification est indisponible, la réduction passe par la barre des tâches.

Le **lancement avec Windows** est facultatif et désactivé par défaut. Activez-le après avoir placé le dossier à son emplacement définitif : le logiciel démarrera réduit près de l'horloge. Après un déplacement du dossier, enregistrez à nouveau les réglages pour actualiser le chemin de démarrage.

## Installer une nouvelle version

La vérification de version consulte GitHub au lancement puis toutes les six heures lorsqu'elle est activée. Elle signale une nouvelle Release stable et donne son lien de téléchargement ; aucun fichier n'est installé automatiquement. Vous pouvez aussi utiliser **Vérifier la version maintenant** dans les réglages.

1. Téléchargez le nouveau ZIP Windows depuis les [Releases](https://github.com/molow159-boop/zaaps-dofus3/releases/latest).
2. Quittez l'ancien Zaaps avec **×** ou **Quitter**. Ganymède peut rester ouvert.
3. Extrayez le nouveau dossier, puis lancez son exécutable.

Les préférences restent dans `%APPDATA%\Zaaps Dofus 3\raccourcis.ini`. Si vous utilisez un `zaaps.csv` personnalisé à côté de l'exécutable, conservez-le lors du remplacement du dossier.

## Affichage au premier plan

Dans les réglages, **Toujours au premier plan** garde les fenêtres de Zaaps au-dessus des autres applications. Cochez ou décochez l’option, puis cliquez sur **Enregistrer**. Le choix s’applique immédiatement et reste mémorisé au prochain lancement. Cette option est désactivée par défaut.

## Signaler une difficulté

Dans les réglages, **Diagnostic — aperçu avant partage** ouvre un résumé modifiable. Dans la vue normale, le bouton **Diagnostic** est aussi disponible directement. Vérifiez l'aperçu avant de le copier dans une [Issue](https://github.com/molow159-boop/zaaps-dofus3/issues).

Le résumé n'inclut ni le texte complet du guide ni le contenu remplacé du presse-papiers. Aucun diagnostic n'est envoyé automatiquement.
