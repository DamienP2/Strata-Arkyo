# Strata Arkyo · maquette v2 · direction « Strates »

Maquette : `maquette/v2/index.html`, un fichier autonome (HTML, CSS et JS en ligne, Google Fonts comme seule ressource externe, aucun script tiers). Captures : `maquette/v2/captures/`.

## 1. Point de départ

Le client trouvait la v1 « trop plate et non visuelle ». C'était juste : la v1 posait tout sur une page blanche, avec des filets noirs, du texte et des pastilles. Rien n'avait d'épaisseur. Le métier lui-même (baies, faces avant, câbles, connecteurs, signaux) ne se voyait presque nulle part, alors qu'aucune autre application ne possède un répertoire visuel aussi riche.

Le client avait déjà écarté deux pistes, le « SaaS générique » (cartes grises arrondies, tableau de bord banal) et le style « industriel ». La v2 garde ce qu'il avait validé : la lecture éditoriale, les très grands titres et la navigation clarifiée.

## 2. Pistes envisagées

| Piste | Idée | Pourquoi elle n'a pas été retenue |
|---|---|---|
| **Régie de nuit** | Interface sombre façon salle de contrôle, lueurs, mur de multiviewer | Pensée pour le sombre seulement, le thème clair reste pauvre. On y retrouve vite les clichés du tableau de bord « gaming » et l'on se rapproche de l'industriel refusé. |
| **Calque technique** | Traits cyan sur fond bleu, esthétique de plan | Une piste monochrome qui étouffe le code couleur des signaux, peu lisible au soleil sur un site, et vite gadget. |
| **Matériel photoréaliste** | Métal brossé, vis et reflets partout | Lourd, c'est l'industriel par une autre porte. Coûteux à maintenir et vieillit mal. |
| **Strates** (retenue) | Le papier éditorial gagne de l'épaisseur : des plaques en couches, de la matière, les signaux en couleur, les objets du métier dessinés et utiles | Elle répond au « plat » par la profondeur et au « pas visuel » par des visuels propres au métier, sans tomber dans le SaaS générique ni dans l'industriel. |

Le nom vient de la marque (*strata*, des couches). Les couches se retrouvent partout : les unités d'une baie, les familles de signaux d'un faisceau, l'élévation des surfaces. Le logo les résume en quatre strates de signal (vidéo, audio, synchro, réseau) posées sur une plaque.

## 3. Principes

1. **Trois matières.**
   - *Papier* : fond mat chaud avec un grain très léger et une lumière de studio en haut à droite.
   - *Plaques* : les surfaces de lecture, avec un liseré, un reflet sur l'arête haute et une ombre douce.
   - *Instruments* : des plaques sombres qui montrent le matériel ou mesurent quelque chose (baies, faces avant, vumètres, spectre, « Prochain tournage »).
2. **Le relief a un sens.** On écrit dans un *puits* (champs, rails des sélecteurs), on actionne une *touche* (boutons, puces de filtre), on lit une *plaque*, on consulte un *instrument*. Le relief n'est jamais décoratif.
3. **Allumé veut dire actif.** L'onglet, la section, le filtre ou le segment actif est une touche encre « allumée » avec une petite LED. En sombre, cette touche est claire.
4. **La couleur porte toujours une information.** Les neuf familles de signaux, les cinq statuts de projet et les témoins (direct, une personne à la fois, alerte) en ont le monopole. Tout le reste est neutre.
5. **On dessine des objets réels.** Chaque visuel montre un vrai objet ou une vraie donnée : la baie avec son occupation réelle, la face avant déduite du type d'appareil, le connecteur qui correspond au signal, les porteuses HF saisies, le planning calculé depuis les dates du projet.
6. **On reste éditorial.** Grands titres en Bricolage Grotesque, phrases d'introduction et listes lisibles comme un document. La matière sert le contenu.

## 4. Jetons

Tous les jetons sont des propriétés personnalisées sur `:root`. Le thème sombre passe par `@media (prefers-color-scheme: dark)` sur `:root:not([data-theme="light"])` et par `:root[data-theme="dark"]` (bouton de thème). `body` a un fond explicite.

### Matières et encre

| Jeton | Clair | Sombre | Usage |
|---|---|---|---|
| `--bg` | `#ECEAE5` | `#0D0E10` | fond de page (papier mat) |
| `--surface` | `#FCFBF9` | `#16181B` | plaques |
| `--surface-2` / `-3` | `#F5F3EF` / `#EEEBE6` | `#1C1F23` / `#23262B` | zones internes, survol |
| `--sunken` | `#E4E1DA` | `#0A0B0D` | puits : champs, rails des sélecteurs |
| `--line` / `--line-2` | `#E1DDD5` / `#C8C3B9` | `#262A2F` / `#3A3E46` | filets |
| `--fg` / `--fg-2` / `--fg-3` | `#141518` / `#3A3D44` / `#5E626A` | `#F1F0ED` / `#C6C9CE` / `#979CA5` | encre |
| `--accent` | `#2943E4` | `#93A4FF` | liens, focus, sélection |
| `--primary` (+ `--primary-bg`) | encre `#16181C` | papier `#F1F0ED` | touche principale, touche « allumée » |
| `--grain` | bruit fractal chaud, alpha 0,2 | bruit clair, alpha 0,07 | matière du fond |

### Témoins, statuts, signaux

| Famille | Jetons |
|---|---|
| Témoins | `--live` (en direct, vert), `--lock` (une personne à la fois, ambre), `--urg` (urgence), chacun avec `-soft` et `-ink` |
| LED de vumètre | `--led-g`, `--led-y`, `--led-r`, segment éteint `--seg-off` |
| Sémantique | `--ok`, `--warn`, `--err`, `--info` avec leurs fonds `-soft` |
| Statuts de projet | `--st-prep`, `--st-pret`, `--st-cours`, `--st-fini`, `--st-annule` (valeurs de la v1 conservées) |
| Signaux | `--sig-v` vidéo, `--sig-a` audio, `--sig-s` synchro, `--sig-n` data, `--sig-f` flux IP, `--sig-i` intercom, `--sig-c` contrôle, `--sig-p` énergie, `--sig-l` lumières (valeurs de la v1 conservées) |
| Instrument | `--inst`, `--inst-hi`, `--inst-lo`, `--inst-fg`, `-2`, `-3`, `--inst-seg` |
| Toiles | `--canvas`, `--dot`, `--node-bg`, `--node-edge`, gaine des repères `--tag-hi`, `--tag-bg`, `--tag-lo`, `--tag-fg`, lueur des câbles `--glow` (aucune en clair, `drop-shadow` en sombre) |
| Illustrations | `--ill-*` (carrosserie, vitrage, pneus, alu, laque, rails, écrans…), redéfinis pour le sombre |
| Galvanomètre | `--gauge-face`, `--gauge-ink`, `--gauge-ndl` (cadran crème en clair, ambre rétroéclairé en sombre) |

### Profondeur

| Jeton | Rôle |
|---|---|
| `--e1` | plaque posée : liseré, reflet haut, ombre courte. Cartes, sections, groupes de navigation. |
| `--e2` | plaque soulevée : survol des cartes, élément choisi. |
| `--e3` | flottant : tiroir, modale, palette, notifications, menu déroulant de section. |
| `--e-inst` | instrument : plaque sombre, ombre profonde, reflet. |
| `--well` | puits : ombre interne. Champs, rails des sélecteurs, compteurs. |
| `--key` / `--key-bg` | touche neutre : dégradé, reflet, ombre portée. |
| `--key-ink` / `--primary-bg` | touche encre « allumée ». |

En sombre, l'élévation se lit à la luminosité : plus une plaque est haute, plus elle est claire, avec un liseré `rgba(255,255,255,.06–.09)`. Les ombres noires restent, car elles détachent du fond.

### Formes, typographie, mouvement

- Rayons : `--r-sm` 8, `--r-md` 12 (touches, champs), `--r-lg` 18 (plaques), `--r-xl` 24 (plaques héros, instruments, modales). Les pastilles et les témoins sont en `999px`. Les faces avant ont un rayon de 2 px, comme du vrai métal.
- `--display` : **Bricolage Grotesque** 500–800. Accueil `clamp(46px, 6,3vw, 94px)`, h1 jusqu'à 54 px, h2 22 px, h3 18 px, approche négative (de −0,048 à −0,012 em).
- `--font` : **Inter** 400–700. Texte courant à 15–16 px, introduction 19–24 px.
- `--mono` : **JetBrains Mono** 500–700. Deux usages :
  1. étiquettes de sérigraphie, en capitales de 10,5 à 11,5 px avec une approche de 0,09 em (sur-titres, libellés de section, en-têtes de tableau) ;
  2. données techniques : repères, numéros de câble, adresses, fréquences, mesures.
- `--ease` : `cubic-bezier(.2,.7,.2,1)`. Durées de 150 ms (états), 220–300 ms (cartes, tiroirs) et 550 ms (entrée de page).

## 5. Règles d'usage

**Élévation**
- Une section est une plaque `--e1` sur le papier. Évitez d'imbriquer des plaques : à l'intérieur d'une plaque, on passe à `--surface-2` avec un liseré interne.
- `--e2` est réservé à ce qui réagit (survol, sélection). `--e3` est réservé à ce qui flotte au-dessus de la page.
- Un champ n'est jamais en relief : c'est un puits. Ce qu'on actionne n'est jamais en creux : c'est une touche.
- Une plaque instrument par écran au plus, en dehors des écrans de conception où la baie et les faces avant sont le sujet.

**Couleur**
- Pas de couleur décorative. Une teinte saturée est un signal, un statut ou un témoin.
- La couleur d'un signal suit l'objet partout : point, liseré du repère de câble, LED de face avant, connecteur, bande d'en-tête d'un nœud, câble.
- Les témoins sont des « tally » : une LED qui rayonne et un texte en mono. En clair, le texte du témoin est assombri (`--tally-k: 82 %`) pour tenir le contraste AA.
- Les alertes clignotent doucement, sauf avec `prefers-reduced-motion`. Rien ne clignote en rouge sans raison.

**Typographie**
- Une seule taille « display » par écran.
- Toutes les données (repères, adresses, fréquences, cotes) sont en mono avec des chiffres tabulaires.
- Une espace fine insécable précède les % dans les textes générés.

**Illustrations et visuels**
- Elles sont dessinées en SVG en ligne, sans aucune image externe, et partagées par un sprite (`<symbol>`) au début du `<body>`.
- Style : aplats et contour de 1,4 px en `--ill-line`, lumière venant d'en haut à gauche, ombre portée en ellipse au sol. Un seul dégradé est permis, pour le vitrage ou l'aluminium.
- Les seules couleurs saturées viennent des signaux : la livrée en strates du car régie, les LED, les câbles.
- Toutes les teintes passent par les jetons `--ill-*`, si bien que les dessins changent avec le thème.
- Un visuel doit montrer quelque chose de vrai. Le type de régie choisit l'illustration, l'occupation des contenants dessine la mini-baie, le nom d'un appareil détermine sa face avant.

**Iconographie**
- Icônes de trait, 24 px, épaisseur 1,7 (le jeu de la v1, complété par `grid`, `list`, `pin`, `cal`, `tablet`, `monitor`, `case`).
- Huit **glyphes de connecteur** dessinés à la main, tracés dans la couleur du signal :

  | Connecteur | Familles de signal |
  |---|---|
  | BNC | vidéo, synchro |
  | XLR | audio |
  | XLR-4 | intercom |
  | XLR-5 | DMX, lumières |
  | RJ45 | data |
  | Fibre LC | flux IP |
  | IEC | énergie |
  | DB9 | contrôle |

**Mouvement**
- Entrée de page : une montée décalée de 12 px, seulement au changement de route. Les mises à jour en direct ne rejouent pas l'animation.
- Le survol soulève les cartes de 3 px.
- Ce qui bouge en continu a un sens : impulsions dans le faisceau de câbles du car régie, LED « en direct » qui pulse, ligne laser du lecteur de code-barres, témoin d'alerte.
- `prefers-reduced-motion` coupe toutes les animations et transitions.

## 6. Composants propres à la v2

- **Vumètre à segments** `vu(p, mode, n)`. Le mode `prog` sert à la progression ; le mode `load` sert à la charge (hauteur, poids, conso, phases) avec des zones vert, ambre et rouge comme un crête-mètre. Il existe en version verticale (console de l'avancement) et horizontale.
- **Galvanomètre** `gauge(p)` : cadran de VU-mètre avec une zone d'objectif au-delà de 90 %.
- **Faces avant** `fpv()` et `.ritem` : 23 types reconnus par le nom, plus un type générique. Deux finitions, anodisé noir et alu brossé. Les oreilles portent des vis. Le décor est composé d'ouïes, de BNC, de XLR, de RJ45, de cages SFP, d'un afficheur LCD, de touches, de baies de disques, de fentes de cartes, d'un écran d'oscilloscope, de prises IEC, d'une poignée ou d'une brosse. Des requêtes de conteneur simplifient le décor quand la face est étroite. La même face sert à la baie, au catalogue, à l'inventaire, aux éléments à placer et aux aperçus.
- **Repère de câble** `.tape` : une gaine thermorétractable blanche, avec un léger volume et un liseré de la couleur du signal. Il existe aussi en SVG sur le synoptique.
- **Témoin** `.tally` : la LED et le texte mono de J-4, d'Indice A, des alertes.
- **Mini-baie** : l'occupation réelle d'un contenant, unité par unité.
- **Planning 30 jours** : préparation hachurée, tournage plein, mise en service en losange, week-ends ombrés, ligne « aujourd'hui ».
- **Spectre HF** : un panneau zoomé par groupe de porteuses, avec une bande de garde de ±0,150 MHz. Les conflits apparaissent en rouge et le panneau se recalcule à chaque saisie.

## 7. Ce qui a changé, vue par vue

- **Coque**
  - Papier mat et lumière de studio.
  - Le logo en quatre strates.
  - Le menu principal devient une touche allumée dans un rail en creux.
  - « En direct » et « Une personne à la fois » deviennent des témoins lumineux.
  - Recherche en puits avec une touche ⌘K, avatars en relief.
  - Sur mobile, le menu plein écran regroupe les sections du projet dans une plaque.
- **Accueil**
  - L'introduction éditoriale est conservée.
  - Une plaque instrument « Prochain tournage » montre le car régie dessiné, son faisceau de câbles animé, le J-4 et un vumètre de complétion.
  - Quatre indicateurs prennent chacun un visuel : mini-baie des projets, vumètre des tâches, galvanomètre de complétion, alertes en témoin cliquable.
  - Un planning des 30 prochains jours remplace la liste « À venir ».
  - Le fil « En direct » est conservé.
  - La carte des statuts reçoit une barre en relief et un « parc » des projets en unités de baie.
- **Projets**
  - Chaque projet devient une fiche illustrée selon le type de régie : car régie, flight-case ou installation fixe.
  - Un liseré lumineux reprend la couleur du statut, avec le J-x en témoin et la complétion en vumètre.
  - Les groupes par statut s'enchaînent côte à côte au lieu d'empiler des lignes vides.
  - Filtres, recherche et bascule Prestations / Intégrations sont inchangés.
- **Vue d'ensemble d'un projet**
  - En-tête héros avec fil d'Ariane, grand titre, témoins et méta. Le car régie CR-2 y sort ses câbles hors de la plaque.
  - Les sections « Organiser / Dessiner / Sur le terrain » deviennent trois plaques de touches ; le groupe Dessiner est teinté ambre et porte un cadenas.
  - L'avancement par domaine devient une console de six voies à vumètres verticaux, et chaque voie mène à l'écran concerné.
  - « Ce qu'il reste à faire », « En ligne » (avec l'appareil utilisé) et le tableau des tâches sont conservés.
- **Inventaire**
  - Les emplacements deviennent des touches avec leur code.
  - Chaque contenant montre sa mini-élévation et des vumètres de charge.
  - Les articles rackables ont une vignette de face avant.
- **Racks**
  - La baie est dessinée en vrai : châssis laqué, rails zingués à trous carrés, faces avant par type avec oreilles et vis, étiquettes d'inventaire, gaffer « BV1 » sur le dessus, ombre au sol, scène éclairée.
  - La scène s'ouvre sur le matériel au lieu des unités vides.
  - La vue de côté et la vue schématique sont conservées et restylées.
  - L'inspecteur montre la face avant de l'élément choisi.
  - La légende utilise les connecteurs.
  - Les éléments à placer montrent leur face avant miniature.
- **Synoptique**
  - Les nœuds deviennent des plaques avec une bande de couleur du signal et une LED.
  - Les ports deviennent des connecteurs (anneau, puis plein quand ils sont câblés).
  - Les câbles ont une gaine qui fait « pont » aux croisements et rayonnent en sombre.
  - Les numéros deviennent des repères thermorétractables.
  - Les calques sont représentés par leurs connecteurs.
- **Plans** : restylés avec les jetons (parquet du terrain, repères des longueurs en gaine). Le fonctionnement est inchangé.
- **Réseau**
  - Les flux SMPTE 2110 reçoivent des essences colorées et des adresses multicast en puits.
  - Le plan HF s'accompagne d'un analyseur de spectre zoomé par groupe de porteuses.
- **Préparation** : la carte de scan affiche un grand compteur, un vumètre de 32 segments et un code-barres avec une ligne laser animée. Les repères deviennent des gaines.
- **Recette** : anneau d'avancement segmenté, compteurs en instruments, sélecteurs d'état à LED (OK vert, réserve ambre, KO rouge).
- **Catalogue**
  - En-tête avec une pile de faces avant et des chiffres clés.
  - Bascule « Vignettes / Liste » (vignettes par défaut) : chaque fiche montre sa face avant à l'échelle, sa famille, ses connecteurs et ses cotes.
  - Le tiroir de détail montre la face avant cotée (19 pouces, 483 mm, hauteur en U) et la connectique en glyphes.
- **Administration** : pile serveur, switch et onduleur en en-tête, plaques système avec icônes en touches, étapes OTA avec LED.
- **Superpositions**
  - Tiroir flottant ; il devient une feuille basse avec une poignée sur mobile.
  - Modale et palette sur fond flouté.
  - L'assistant illustre les trois types de régie.
  - Toasts en touche encre avec LED.
- **Mobile, de 360 à 860 px**
  - Barre compacte.
  - Sélecteur de section en plaque déroulante, avec la position (« 3 / 8 »).
  - Puces de filtre défilantes, console de l'avancement à l'horizontale, tableaux en fiches empilées.
  - Aucun défilement horizontal de page à 360, 390, 768, 1024 et 1600 px, dans les deux thèmes.

## 8. Accessibilité

- Contrastes AA vérifiés sur les paires de jetons, dans les deux thèmes. Exemples : `--fg-3` sur `--sunken` à 4,69 en clair, texte des témoins entre 5,8 et 6 en clair, `--inst-fg-3` sur l'instrument à 5,47 en sombre.
- Anneau de focus de 2 px en `--accent`, couleur de LED sur les instruments sombres.
- Cibles tactiles de 32 à 44 px (WCAG 2.2 AA en exige 24), 44 px pour les entrées du sélecteur de section sur mobile.
- `role="img"` et `aria-label` sur les vumètres, le galvanomètre et le spectre, `aria-pressed` sur les bascules, `aria-current` sur la section active. Les visuels décoratifs sont en `aria-hidden`.
- `prefers-reduced-motion` est respecté.

## 9. Limites et suites conseillées

- **Faces avant génériques.** Elles sont plausibles mais ne reproduisent pas les modèles des constructeurs. Prochaine étape : une bibliothèque de faces SVG par fiche du catalogue (avant et arrière), importable, avec repli sur le rendu générique actuel.
- **Trois illustrations seulement** (car régie, flight-case, installation fixe). Il en faudra pour les valises, les plateaux, les lieux, les états vides et la page de connexion.
- **Plans** : ils ont été restylés mais pas repensés. La cartographie et le plan de feu mériteraient leur propre passe (fonds de plan, symboles normalisés).
- **Performances** : grain, ombres portées des nœuds et lueur des câbles sont légers à l'échelle de la maquette (11 nœuds, 15 câbles). Il faudra les mesurer sur de gros synoptiques (plus de 1 000 câbles) et couper les lueurs au-delà d'un seuil.
- **Vérifications terrain** : tablette en extérieur (lisibilité des puits clairs), lecteur d'écran sur les écrans de conception, impression des planches.
- **Captures** : le grain alourdit les PNG (environ 16 Mo pour les dix captures en ×2).
