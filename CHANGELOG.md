# Historique des versions

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
