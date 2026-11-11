# Maquette interactive — Strata Arkyo

Prototype cliquable de la web app (HTML/CSS/JS sans dépendance), réalisé à partir du cahier des charges fonctionnel. Direction visuelle : éditorial clair (page blanche, navigation par onglets, grands titres, listes sans cadres).

Décisions intégrées :
- Serveur **Ubuntu uniquement**, service systemd, moteur **Rust** (`strata-engine`).
- **Mises à jour OTA** signées (ed25519 + SHA-256), essai à blanc, bascule, retour automatique après 3 échecs de démarrage.
- **Temps réel** sur inventaire, recette, tâches, catalogue et accueil (simulé toutes les 10 s).
- **Écrans de conception** (racks, synoptique, plans) : pas de diffusion en direct, un seul éditeur à la fois (verrou « avoir la main », bouton « Voir comme Léa » pour simuler la vue d'un autre membre).
- Web app responsive (barre latérale sur bureau, barre d'onglets sur mobile), thèmes clair et sombre.

`index.html` est écrit pour être publié comme artefact (sans `<!doctype>` ni `<html>` : le squelette est ajouté à la publication).
