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
  return 'blank';
}
DECOR.blank = '';
