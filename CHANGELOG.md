# Historique des versions

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
