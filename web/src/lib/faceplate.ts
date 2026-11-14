// Faces avant des équipements dans l'élévation : type déduit du nom, finition alu ou
// sombre, décor (connecteurs, afficheurs, ouïes…). Repris de la maquette v2.

export const FACE: [RegExp, string][] = [[/Onduleur/,'ups'],[/Passe-câble/,'brush'],[/Panel BNC/,'bnc'],[/Panel XLR/,'xlr'],[/Panel RJ45/,'rjp'],[/Grille/,'router'],[/Mélangeur/,'mixer'],[/replay/i,'replay'],[/openGear/,'cards'],[/Switch/,'switch'],[/forme d’onde/,'scope'],[/synchro|PTP/i,'sync'],[/Multiviewer/,'mv'],[/Plaque aérée/,'vent'],[/PDU/,'pdu'],[/Enregistreur/,'rec'],[/Tiroir/,'drawer'],[/intercom/i,'intercom'],[/CCU/,'ccu'],[/Stagebox/,'stage'],[/Désembeddeur|Distributeur/,'dist'],[/Pare-feu|Serveur/,'server'],[/Récepteur HF|antennes/,'rx']];
export const ALU = new Set(['router','mixer','scope','sync','rec','intercom']);
export const faceType = (n: string): string => (FACE.find(([re]) => re.test(n)) ?? [0, 'gen'])[1] as string;
const SCOPE = '<svg viewBox="0 0 70 20" preserveAspectRatio="none" aria-hidden="true"><path d="M2 15l4-9 3 9 3-10 3 10 3-8 3 8 3-11 3 11 4-6 4 6 3-9 3 9 3-7 3 7 3-10 3 10 3-6 3 6" fill="none" stroke="#5CF29A" stroke-width="1.1"/></svg>';
// Décors : une silhouette simple et typique de chaque famille d'appareils (ce qu'on voit en
// façade sur la majorité des modèles), sans détail superflu. Les tailles suivent la hauteur
// de la façade (unités de conteneur), donc un 1U et un 3U restent proportionnés.
const rep = (n: number, h: string) => Array.from({ length: n }, () => h).join('');
const MOSAIC = '<i class="d-mosaic"><i></i><i></i><i></i><i></i></i>';
const TRANSPORT = '<i class="d-trans"><i></i><i></i><i></i><i class="rec"></i></i>';
export const DECOR: Record<string, string> = {
  switch: `<i class="d-ports">${rep(3, '<i class="d-rjg"></i>')}</i><i class="d-sfp"></i><i class="d-leds s"></i>`,
  server: '<i class="d-pwr"></i><i class="d-usb"></i><i class="d-bays"></i>',
  router: '<i class="lcd">12G</i><i class="d-xkeys"></i><i class="d-pwr"></i>',
  mixer: '<i class="d-grille"></i><i class="lcd">M/E</i><i class="d-pwr"></i>',
  rec: `<i class="d-thumb">${'<svg viewBox="0 0 7 4" preserveAspectRatio="none"><rect width="1" height="3" fill="#bfbfbf"/><rect x="1" width="1" height="3" fill="#bfbf00"/><rect x="2" width="1" height="3" fill="#00bfbf"/><rect x="3" width="1" height="3" fill="#00bf00"/><rect x="4" width="1" height="3" fill="#bf00bf"/><rect x="5" width="1" height="3" fill="#bf0000"/><rect x="6" width="1" height="3" fill="#0000bf"/><rect y="3" width="7" height="1" fill="#131313"/></svg>'}</i><i class="d-sd"></i><i class="d-sd"></i>${TRANSPORT}`,
  replay: '<i class="lcd">REC</i><i class="d-caddies"></i><i class="d-recbtn"></i>',
  mv: `${MOSAIC}<i class="d-leds"></i>`,
  cards: '<i class="d-cards"></i>',
  sync: '<i class="d-tc">10:00:00:00</i><i class="d-btns"></i>',
  dist: '<i class="d-leds w"></i><i class="d-pwr"></i>',
  ccu: '<i class="lcd">CAM</i><i class="d-knob"></i><i class="d-knob"></i><i class="d-joy"></i>',
  intercom: '<i class="d-ic"></i><i class="d-knob"></i>',
  rx: '<i class="d-blcd">CH1</i><i class="d-blcd">CH2</i><i class="d-knob"></i><i class="d-jack"></i>',
  stage: '<i class="d-xlr2"></i>',
  scope: `<i class="scr">${SCOPE}</i><i class="d-knob"></i><i class="d-knob"></i>`,
  ups: '<i class="lcd amb">230V</i><i class="d-btns s"></i><i class="perf"></i>',
  pdu: '<i class="d-brk"></i><i class="d-sock"></i>',
  rjp: '<i class="d-keystone"></i>',
  bnc: '<i class="d-bnc"></i>',
  xlr: '<i class="d-xlr"></i>',
  gen: '<i class="d-grille s"></i><i class="d-pwr"></i>',
  vent: '<i class="d-hex"></i>',
  brush: '<i class="d-brush"></i>',
  drawer: '<i class="d-drawer"><i></i></i>',
  shelf: '<i class="d-shelf"></i>',
};
/** Décor HTML (statique, sans donnée saisie) d'un type de face. */
export const decor = (t: string): string => DECOR[t] ?? DECOR.gen!;

/** Face d'un accessoire : d'après sa nature (plaque, étagère, tiroir, laçage), sinon son libellé. */
export function accFace(e: { label: string; accessory?: { kind: string; variant?: string } | null }): string {
  const a = e.accessory;
  if (a?.kind === 'plate') return a.variant === 'brush' ? 'brush' : a.variant === 'vented' ? 'vent' : 'blank';
  if (a?.kind === 'shelf') return 'shelf';
  if (a?.kind === 'drawer') return 'drawer';
  if (a?.kind === 'lacing_bar') return 'lacing';
  return accessoryFace(e.label);
}
/** Face d'un accessoire du rack d'après son libellé. */
export function accessoryFace(label: string): string {
  if (/passe-câble/i.test(label)) return 'brush';
  if (/aérée/i.test(label)) return 'vent';
  if (/tiroir/i.test(label)) return 'drawer';
  if (/étagère/i.test(label)) return 'shelf';
  if (/laçage/i.test(label)) return 'lacing';
  return 'blank';
}
DECOR.blank = '<i class="d-blank"></i>';
DECOR.lacing = '<i class="lace"></i>';

// ----- Visuel en baie -----
// Choisi dans la fiche équipement, sinon déduit du type du catalogue (et de son type parent),
// sinon du nom. Sert à la vue réelle et à la 3D.
import { refById } from './catalog.svelte';

/** Visuels proposés dans la fiche équipement. */
export const VISUALS: [string, string][] = [
  ['monitor', 'Moniteur · 1 écran'], ['monitor2', 'Moniteur · 2 écrans'], ['monitor3', 'Moniteur · 3 écrans'], ['monitor4', 'Moniteur · 4 écrans'], ['switch', 'Switch réseau'], ['server', 'Serveur / ordinateur'], ['router', 'Grille / routeur vidéo'],
  ['mixer', 'Mélangeur / console'], ['rec', 'Enregistreur'], ['replay', 'Serveur vidéo / replay'], ['mv', 'Multiviewer'],
  ['cards', 'Châssis de cartes / convertisseurs'], ['sync', 'Synchro / timecode'], ['dist', 'Distributeur'], ['ccu', 'CCU / voie caméra'],
  ['intercom', 'Intercom'], ['rx', 'Récepteur HF'], ['stage', 'Stagebox / E-S audio'], ['scope', 'Forme d’onde'],
  ['ups', 'Onduleur'], ['pdu', 'PDU'], ['rjp', 'Panel RJ45'], ['bnc', 'Panel BNC'], ['xlr', 'Panel XLR'], ['gen', 'Générique'],
];
const BY_TYPE: [RegExp, string][] = [
  [/moniteur|écran|monitor|lcd|oled/i, 'monitor'], [/multiviewer/i, 'mv'], [/forme d.onde|waveform|wfm|technique/i, 'scope'],
  [/switch|routeur réseau|firewall|sd-wan|vpn|bonded/i, 'switch'], [/routeur vid|grille|matrice vid|^ip \/|^sdi$/i, 'router'],
  [/serveur vid|replay|ralenti|playout/i, 'replay'], [/enregistreur|ingest/i, 'rec'],
  [/serveur|nas|san|stockage|gpu|^pc$|^mac$|ordinateur|time server/i, 'server'],
  [/mélangeur|console|surface de contr|pupitre/i, 'mixer'], [/sync|timecode|ptp|genlock|black burst|tri-level/i, 'sync'],
  [/onduleur|ups|batterie/i, 'ups'], [/pdu|distribution motoris|inverseur/i, 'pdu'], [/intercom|beltpack|station|4-wire/i, 'intercom'],
  [/ccu|voie caméra/i, 'ccu'], [/stagebox|i\/o|splitter audio/i, 'stage'], [/amplificateur de distribution|distributeur|splitter/i, 'dist'],
  [/récepteur|émetteur|antenne|iem|micro hf|combineur/i, 'rx'],
  [/convertisseur|frame sync|embedder|processeur|gateway|encodeur|décodeur|up\/down/i, 'cards'], [/patch panel/i, 'rjp'],
];
/** Visuel d'un équipement : choisi, sinon selon son type (sous-type puis type), sinon selon son nom. */
export function visualOf(e: { visual?: string | null; type_id?: string | null; label: string }): string {
  if (e.visual) return e.visual;
  const t = e.type_id ? refById('types', e.type_id) : null;
  const names = [t?.name, t?.parent_id ? refById('types', t.parent_id)?.name : null].filter(Boolean) as string[];
  let v: string | null = null;
  for (const n of names) { const hit = BY_TYPE.find(([re]) => re.test(n)); if (hit) { v = hit[1]; break; } }
  if (!v && /moniteur|monitor|écran/i.test(e.label)) v = 'monitor';
  v ??= faceType(e.label);
  // Moniteurs en rack : nombre d'écrans d'après le nom (Dual, 2×7″, Triple, Quad…).
  if (v === 'monitor') {
    const n = /quad|4\s?[x×]/i.test(e.label) ? 4 : /tri(ple)?\b|3\s?[x×]/i.test(e.label) ? 3 : /dual|double|duo|2\s?[x×]/i.test(e.label) ? 2 : 1;
    if (n > 1) v = `monitor${n}`;
  }
  return v;
}
/** Nombre d'écrans d'un visuel de moniteur (0 si ce n'en est pas un). */
export const screensOf = (v: string): number => (v === 'monitor' ? 1 : /^monitor([2-4])$/.exec(v) ? Number(v.slice(7)) : 0);
// Mire SMPTE (barres, contre-barres, pluge), un peu atténuée comme sur un vrai écran.
const MON = '<svg viewBox="0 0 70 40" preserveAspectRatio="none" aria-hidden="true">' +
  ['#bfbfbf', '#bfbf00', '#00bfbf', '#00bf00', '#bf00bf', '#bf0000', '#0000bf'].map((c, i) => `<rect x="${i * 10}" y="0" width="10.2" height="27" fill="${c}"/>`).join('') +
  ['#0000bf', '#131313', '#bf00bf', '#131313', '#00bfbf', '#131313', '#bfbfbf'].map((c, i) => `<rect x="${i * 10}" y="27" width="10.2" height="3" fill="${c}"/>`).join('') +
  '<rect x="0" y="30" width="12.5" height="10" fill="#00214c"/><rect x="12.5" y="30" width="12.5" height="10" fill="#fff"/><rect x="25" y="30" width="12.5" height="10" fill="#32006a"/><rect x="37.5" y="30" width="32.5" height="10" fill="#131313"/></svg>';
const screens = (n: number) => `<i class="mwrap" style="--n:${n}">${Array.from({ length: n }, () => `<i class="mscr">${MON}<i class="umd"></i></i>`).join('')}</i><i class="mkeys"></i>`;
DECOR.monitor = screens(1);
DECOR.monitor2 = screens(2);
DECOR.monitor3 = screens(3);
DECOR.monitor4 = screens(4);
