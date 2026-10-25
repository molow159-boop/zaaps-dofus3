# Historique des versions

## 1.1.5

- Reconnaissance des destinations où la dimension suit les coordonnées, comme « Ensuite allez en [2,2] dans la dimension Srambad ».
- Préparation du trajet vers la Tour des Voyageurs Dimensionnels pour ces formulations, y compris lorsque les éléments sont séparés dans la lecture de Ganymède.
- Tests de non-régression sur les cinq dimensions, les mentions hors destination et la copie automatique unique après confirmation.

## 1.1.4

- Option « Toujours au premier plan » dans les réglages, désactivée par défaut et mémorisée entre les lancements.
- Application à la fenêtre principale, au gestionnaire, aux réglages et au diagnostic dès l'enregistrement, avec retour à l'affichage normal lorsque l'option est désactivée.

## 1.1.3

- Alignement sur les bords visibles des fenêtres, sans l'espace des bordures invisibles de Windows.
- Largeur du compagnon automatiquement ajustée à celle de Ganymède en haut et en bas ; hauteur compacte conservée.
- Calcul des positions en pixels physiques pour éviter les décalages liés à l'échelle d'affichage.
- Reprise de l'accrochage après un mouvement ou redimensionnement lorsqu'un déplacement précédent n'avait pas été confirmé.
- Tests sur les bordures, les échelles d'affichage, le suivi de largeur et la vue compacte de 320 à 460 pixels de large.

## 1.1.2

- Vue accrochée compacte de 225 pixels de haut, avec bouton « Afficher Zaaps » ouvrant le gestionnaire indépendamment ; réglages et diagnostic dans des fenêtres séparées.
- Correction de l'accrochage lorsque la fenêtre technique de l'icône Ganymède était comptée comme une seconde fenêtre principale.
- Exclusion des fenêtres d'outil, sans activation ou appartenant à une autre fenêtre ; conservation du refus en présence de deux véritables fenêtres principales.
- Tests de non-régression sur la fenêtre principale et la fenêtre technique de notification.

## 1.1.1

- Accrochage facultatif de Zaaps à Ganymède : en haut, en bas, à gauche ou à droite.
- Déplacement lié dans les deux sens, alignement conservé après redimensionnement et côté mémorisé.
- Pause lorsque l'une des fenêtres est réduite, masquée ou maximisée ; contrôle de la place disponible à l'écran.
- Tests de non-régression sur l'alignement, les déplacements, le redimensionnement et l'enregistrement des préférences.

## 1.1.0

- Vue compacte mémorisée avec numéro d'étape, Zaap, destination et trois boutons de copie ; gestionnaire complet dépliable.
- Réduction près de l'horloge avec lecture Ganymède et raccourcis actifs ; menu pour afficher, suspendre Auto, recopier le trajet ou quitter.
- Deux raccourcis configurables supplémentaires pour Auto et l'affichage de la fenêtre.
- Confirmation de copie visuelle, sonore ou silencieuse, et avertissement lorsqu'une autre copie remplace la commande.
- Démarrage avec Windows facultatif, réduit près de l'horloge.
- Vérification des nouvelles versions GitHub avec téléchargement manuel.
- Aperçu de diagnostic modifiable avant copie, sans envoi automatique.
- Conservation des raccourcis existants, des 45 Zaaps, des alias et des règles de trajet dimensionnel.

## 1.0.3

- Préparation du trajet vers la Tour des Voyageurs Dimensionnels en [-22,-24], via le Zaap Routes Rocailleuses en [-20,-20], lorsque la prochaine destination annoncée est Enutrosor, Srambad, Xélorium, Ecaflipus ou Osavora.
- Distinction entre une dimension mentionnée dans l'action en cours et la prochaine destination : une mention de la Dimension Obscure ne bloque plus un trajet suivant clairement identifié.
- Tests de non-régression sur les dimensions, les textes d'accessibilité fragmentés, les raccourcis et la copie automatique unique par changement d'étape.

## 1.0.2

- Correction de la détection de « Cimetière primitif », désormais distingué du Zaap « Cimetière » d'Incarnam.
- Priorité au nom complet lorsque deux noms se recouvrent dans une même mention ; les mentions distinctes restent refusées en cas d'ambiguïté.
- Tests de non-régression sur l'étape, les deux cimetières, les Zaapis et les mentions ambiguës.

## 1.0.1

- Reconnaissance de « Village ensevelli », variante utilisée dans Ganymède, comme le Zaap « Village enseveli ».
- Conservation des coordonnées du Zaap et prise en charge des deux écritures pour les raccourcis et le mode automatique.
- Tests de non-régression sur les variantes du nom, les Zaapis et les destinations ambiguës.

## 1.0.0

- Répertoire intégré de 45 zaaps, recherche et copie rapide.
- Détection du Zaap et de la destination à partir des étapes Ganymède.
- Trajet complet avec double `/zaap` puis `/travel`, ou `/travel` seul selon les données disponibles.
- Raccourcis Ctrl+J, Ctrl+K et Ctrl+L personnalisables.
- Mode automatique avec confirmation des données et copie unique par nouvelle étape reconnue.
- Reconnaissance des alias Astrub, Bonta et Brâkmar ; Zaapis ignorés.
- Interface responsive, redimensionnement libre, coins arrondis et icône Windows.
- Tests de non-régression et validation automatisée sous Windows.
