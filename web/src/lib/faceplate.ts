// Faces avant des équipements dans l'élévation : type déduit du nom, finition alu ou
// sombre, décor (connecteurs, afficheurs, ouïes…). Repris de la maquette v2.

export const FACE: [RegExp, string][] = [[/Onduleur/,'ups'],[/Passe-câble/,'brush'],[/Panel BNC/,'bnc'],[/Panel XLR/,'xlr'],[/Panel RJ45/,'rjp'],[/Grille/,'router'],[/Mélangeur/,'mixer'],[/replay/i,'replay'],[/openGear/,'cards'],[/Switch/,'switch'],[/forme d’onde/,'scope'],[/synchro|PTP/i,'sync'],[/Multiviewer/,'mv'],[/Plaque aérée/,'vent'],[/PDU/,'pdu'],[/Enregistreur/,'rec'],[/Tiroir/,'drawer'],[/intercom/i,'intercom'],[/CCU/,'ccu'],[/Stagebox/,'stage'],[/Désembeddeur|Distributeur/,'dist'],[/Pare-feu|Serveur/,'server'],[/Récepteur HF|antennes/,'rx']];
export const ALU = new Set(['router','mixer','scope','sync','rec','intercom']);
export const faceType = (n: string): string => (FACE.find(([re]) => re.test(n)) ?? [0, 'gen'])[1] as string;
const SCOPE = '<svg viewBox="0 0 70 20" preserveAspectRatio="none" aria-hidden="true"><path d="M2 15l4-9 3 9 3-10 3 10 3-8 3 8 3-11 3 11 4-6 4 6 3-9 3 9 3-7 3 7 3-10 3 10 3-6 3 6" fill="none" stroke="#5CF29A" stroke-width="1.1"/></svg>';
export const DECOR: Record<string, string> = {ups:'<i class="lcd amb">230V</i><i class="perf"></i><i class="pwr"></i>',brush:'<i class="brush"></i>',bnc:'<i class="bnc r2"></i>',xlr:'<i class="xlr"></i>',rjp:'<i class="rj"></i>',
 router:'<i class="lcd">12G</i><i class="keys"></i><i class="pwr"></i>',mixer:'<i class="vent"></i><i class="lcd">M/E</i><i class="pwr"></i>',replay:'<i class="bays"></i><i class="lcd">REC</i><i class="pwr"></i>',
 cards:'<i class="slots"></i>',switch:'<i class="rj r2"></i><i class="sfp"></i>',scope:`<i class="scr">${SCOPE}</i><i class="knob"></i><i class="knob"></i>`,sync:'<i class="lcd">PTP</i><i class="pwr"></i>',
 mv:'<i class="lcd">MV</i><i class="vent"></i>',vent:'<i class="vent w"></i>',pdu:'<i class="sock"></i>',rec:'<i class="ssd"></i><i class="lcd">REC</i>',drawer:'<i class="handle"></i>',
 intercom:'<i class="lcd">IC</i><i class="keys"></i>',ccu:'<i class="lcd">CAM</i><i class="knob"></i><i class="knob"></i><i class="keys"></i>',stage:'<i class="xlr r2"></i>',dist:'<i class="bnc"></i>',
 server:'<i class="bays"></i><i class="pwr"></i>',rx:'<i class="lcd">606.1</i><i class="knob"></i>',gen:'<i class="vent"></i>'};
/** Décor HTML (statique, sans donnée saisie) d'un type de face. */
export const decor = (t: string): string => DECOR[t] ?? DECOR.gen!;

/** Face d'un accessoire du rack. */
export function accessoryFace(label: string): string {
  if (/passe-câble/i.test(label)) return 'brush';
  if (/aérée/i.test(label)) return 'vent';
  if (/tiroir/i.test(label)) return 'drawer';
  if (/étagère/i.test(label)) return 'drawer';
  if (/laçage/i.test(label)) return 'lacing';
  return 'blank';
}
DECOR.blank = '';
DECOR.lacing = '<i class="lace"></i>';

// ----- Visuel en baie -----
// Choisi dans la fiche équipement, sinon déduit du type du catalogue (et de son type parent),
// sinon du nom. Sert à la vue réelle et à la 3D.
import { refById } from './catalog.svelte';

/** Visuels proposés dans la fiche équipement. */
export const VISUALS: [string, string][] = [
  ['monitor', 'Moniteur'], ['switch', 'Switch réseau'], ['server', 'Serveur / ordinateur'], ['router', 'Grille / routeur vidéo'],
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
  for (const n of names) { const hit = BY_TYPE.find(([re]) => re.test(n)); if (hit) return hit[1]; }
  return faceType(e.label);
}
const MON = '<svg viewBox="0 0 70 20" preserveAspectRatio="none" aria-hidden="true">' +
  ['#c0c0c0', '#c0c000', '#00c0c0', '#00c000', '#c000c0', '#c00000', '#0000c0'].map((c, i) => `<rect x="${i * 10}" y="0" width="10" height="14" fill="${c}"/>`).join('') +
  '<rect x="0" y="14" width="70" height="6" fill="#111"/></svg>';
DECOR.monitor = `<i class="scr mon">${MON}</i><i class="knob"></i>`;
