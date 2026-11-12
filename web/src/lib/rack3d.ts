// Vue 3D réaliste d'une baie ou d'un flight-case : WebGL (three.js), matériaux physiques,
// éclairage d'environnement, ombres douces, halo des écrans et voyants. Tout est construit
// à partir des données : dimensions de l'élément, équipements posés (U, face, largeur),
// connecteurs déclarés au catalogue, panels. Unités : mètres ; Y vers le haut, +Z = avant.

import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { RoomEnvironment } from 'three/examples/jsm/environments/RoomEnvironment.js';
import { EffectComposer } from 'three/examples/jsm/postprocessing/EffectComposer.js';
import { RenderPass } from 'three/examples/jsm/postprocessing/RenderPass.js';
import { UnrealBloomPass } from 'three/examples/jsm/postprocessing/UnrealBloomPass.js';
import { OutputPass } from 'three/examples/jsm/postprocessing/OutputPass.js';
import { RoundedBoxGeometry } from 'three/examples/jsm/geometries/RoundedBoxGeometry.js';

export const U = 0.04445;
const W19 = 0.4826; // largeur d'une face 19"
const OPEN = 0.4508; // passage entre montants

export interface Port3D { face: string; connector: string; quantity: number; color: string | null; label: string | null }
export interface El3D {
  id: string; label: string; tag: string | null; nature: string; height_u: number; depth_mm: number | null; function: string | null;
  brand: string | null; face: 'front' | 'rear' | 'rear_inner'; u_start: number; width: 'full' | 'half' | 'third' | 'quarter'; col: number;
  kind: string; // type de face (faceplate.ts)
  alu: boolean; accessory: any | null; ports: Port3D[]; fnColor: string;
  shelf_id: string | null;
}
export interface Box3DSpec { kind: string; name: string; units: number; width_mm: number | null; depth_mm: number | null; height_mm: number | null; rear_rails_mm: number | null; wheels: boolean }
export interface Hit { label: string; tag: string | null; u: number; height: number; face: string }

// ----- Textures procédurales -----

function canvas(w: number, h: number): [HTMLCanvasElement, CanvasRenderingContext2D] {
  const c = document.createElement('canvas'); c.width = w; c.height = h;
  return [c, c.getContext('2d')!];
}
function rng(seed: number) { let s = seed >>> 0 || 1; return () => ((s = (s * 1664525 + 1013904223) >>> 0) / 4294967296); }
function tex(c: HTMLCanvasElement, srgb = true, repeat?: [number, number]): THREE.CanvasTexture {
  const t = new THREE.CanvasTexture(c);
  if (srgb) t.colorSpace = THREE.SRGBColorSpace;
  t.anisotropy = 8;
  if (repeat) { t.wrapS = t.wrapT = THREE.RepeatWrapping; t.repeat.set(repeat[0], repeat[1]); }
  return t;
}
/** Bruit fin (rugosité, relief) autour d'un gris. */
function noise(size: number, base: number, amp: number, seed = 7, streak = 0): HTMLCanvasElement {
  const [c, g] = canvas(size, size); const d = g.createImageData(size, size); const r = rng(seed);
  let row = 0;
  for (let y = 0; y < size; y++) {
    if (streak) row = (r() - 0.5) * streak;
    for (let x = 0; x < size; x++) {
      const v = Math.max(0, Math.min(255, base + (r() - 0.5) * amp + row + (streak ? (r() - 0.5) * streak * 0.6 : 0)));
      const i = (y * size + x) * 4; d.data[i] = d.data[i + 1] = d.data[i + 2] = v; d.data[i + 3] = 255;
    }
  }
  g.putImageData(d, 0, 0); return c;
}
/** Peau martelée des panneaux de flight-case (relief). */
function hammered(size = 512): HTMLCanvasElement {
  const [c, g] = canvas(size, size); const r = rng(42);
  g.fillStyle = '#808080'; g.fillRect(0, 0, size, size);
  for (let i = 0; i < 2600; i++) {
    const x = r() * size, y = r() * size, rad = 2 + r() * 7;
    const gr = g.createRadialGradient(x, y, 0, x, y, rad);
    const v = Math.round(90 + r() * 80);
    gr.addColorStop(0, `rgba(${v},${v},${v},.55)`); gr.addColorStop(1, 'rgba(128,128,128,0)');
    g.fillStyle = gr; g.beginPath(); g.arc(x, y, rad, 0, Math.PI * 2); g.fill();
  }
  return c;
}
/** Tôle perforée (porte) : alpha, trous hexagonaux. */
function perforation(): HTMLCanvasElement {
  const [c, g] = canvas(64, 112);
  g.fillStyle = '#fff'; g.fillRect(0, 0, 64, 112); g.fillStyle = '#000';
  const hole = (x: number, y: number) => { g.beginPath(); for (let k = 0; k < 6; k++) { const a = Math.PI / 6 + k * Math.PI / 3; g.lineTo(x + 13 * Math.cos(a), y + 13 * Math.sin(a)); } g.closePath(); g.fill(); };
  hole(16, 14); hole(48, 14); hole(0, 42); hole(32, 42); hole(64, 42); hole(16, 70); hole(48, 70); hole(0, 98); hole(32, 98); hole(64, 98);
  return c;
}
/** Montant 19" : trous carrés (alpha) et numéros de U (couleur), une U par motif. */
function railTextures(units: number): { map: THREE.CanvasTexture; alpha: THREE.CanvasTexture } {
  const px = 40; const [c, g] = canvas(64, Math.min(8192, units * px)); const [a, ga] = canvas(64, c.height);
  const k = c.height / (units * px);
  g.fillStyle = '#b9bcc1'; g.fillRect(0, 0, 64, c.height); ga.fillStyle = '#fff'; ga.fillRect(0, 0, 64, c.height);
  for (let u = 0; u < units; u++) {
    const y0 = (units - u - 1) * px * k;
    for (const off of [6.35 / 44.45, 22.225 / 44.45, 38.1 / 44.45]) { const yy = y0 + (1 - off) * px * k - 4 * k; ga.fillStyle = '#000'; ga.fillRect(34, yy, 9, 9 * k); }
    g.fillStyle = '#2a2c30'; g.font = `bold ${Math.max(8, 14 * k)}px Helvetica, Arial`; g.textAlign = 'center';
    g.fillText(String(u + 1), 16, y0 + px * k * 0.62);
    g.fillStyle = 'rgba(0,0,0,.25)'; g.fillRect(4, y0 + px * k - 1, 20, 1);
  }
  return { map: tex(c), alpha: tex(a, false) };
}

// ----- Faces avant des équipements -----

function roundRect(g: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) { g.beginPath(); g.roundRect(x, y, w, h, r); }

/** Face d'un équipement : couleur, émission (écrans, voyants) et rugosité. */
function faceCanvases(e: El3D, wPx: number, hPx: number) {
  const [c, g] = canvas(wPx, hPx); const [ce, ge] = canvas(wPx, hPx); const [cr, gr] = canvas(wPx, hPx);
  const r = rng(e.label.length * 97 + e.u_start);
  const alu = e.alu;
  // Fond : alu brossé ou anodisé noir.
  const base = alu ? '#c3c6cb' : e.nature === 'accessory' ? '#1d1e21' : '#202226';
  g.fillStyle = base; g.fillRect(0, 0, wPx, hPx);
  for (let y = 0; y < hPx; y++) { const v = (r() - 0.5) * (alu ? 34 : 10); g.fillStyle = `rgba(${v > 0 ? 255 : 0},${v > 0 ? 255 : 0},${v > 0 ? 255 : 0},${Math.abs(v) / 255})`; g.fillRect(0, y, wPx, 1); }
  ge.fillStyle = '#000'; ge.fillRect(0, 0, wPx, hPx);
  gr.fillStyle = alu ? '#555' : '#9a9a9a'; gr.fillRect(0, 0, wPx, hPx);
  const ear = e.width === 'full' ? wPx * 0.035 : 0;
  // Oreilles et vis de fixation.
  if (ear) {
    for (const x of [ear / 2, wPx - ear / 2]) {
      g.fillStyle = alu ? 'rgba(0,0,0,.08)' : 'rgba(255,255,255,.04)'; g.fillRect(x - ear / 2, 0, ear, hPx);
      // Deux vis par oreille en 1U (haut et bas), une par U au-delà.
      const nU = Math.max(1, Math.round(e.height_u));
      const ys = nU === 1 ? [0.27, 0.73] : Array.from({ length: nU }, (_, k) => (k + 0.5) / nU);
      for (const fy of ys) {
        const y = hPx * fy;
        const sg = g.createRadialGradient(x - 2, y - 2, 1, x, y, ear * 0.28);
        sg.addColorStop(0, '#f2f3f5'); sg.addColorStop(1, '#6b6e74'); g.fillStyle = sg; g.beginPath(); g.arc(x, y, ear * 0.26, 0, Math.PI * 2); g.fill();
        g.strokeStyle = '#3a3c40'; g.lineWidth = 2; g.beginPath(); g.moveTo(x - ear * 0.16, y); g.lineTo(x + ear * 0.16, y); g.moveTo(x, y - ear * 0.16); g.lineTo(x, y + ear * 0.16); g.stroke();
      }
    }
  }
  const x0 = ear + wPx * 0.02, x1 = wPx - ear - wPx * 0.02;
  const t = e.kind;
  // Ouïes de ventilation.
  const vents = (xa: number, xb: number) => {
    const n = Math.floor((xb - xa) / 9);
    for (let i = 0; i < n; i++) { const xx = xa + i * 9; roundRect(g, xx, hPx * 0.18, 5, hPx * 0.64, 2.5); g.fillStyle = '#050506'; g.fill(); gr.fillStyle = '#fff'; gr.fillRect(xx, hPx * 0.18, 5, hPx * 0.64); }
  };
  const lcd = (x: number, w: number, text: string, color: string) => {
    const h = Math.min(hPx * 0.46, 46), y = (hPx - h) / 2;
    roundRect(g, x - 2, y - 2, w + 4, h + 4, 4); g.fillStyle = '#0b0c0d'; g.fill();
    roundRect(g, x, y, w, h, 3); g.fillStyle = '#071410'; g.fill();
    ge.fillStyle = color; ge.font = `bold ${Math.round(h * 0.55)}px Courier, monospace`; ge.textBaseline = 'middle'; ge.fillText(text, x + 8, y + h / 2);
    g.fillStyle = color; g.globalAlpha = 0.85; g.font = ge.font; g.textBaseline = 'middle'; g.fillText(text, x + 8, y + h / 2); g.globalAlpha = 1;
    gr.fillStyle = '#111'; gr.fillRect(x, y, w, h);
  };
  const led = (x: number, y: number, color: string) => {
    g.fillStyle = '#111'; g.beginPath(); g.arc(x, y, 5, 0, Math.PI * 2); g.fill();
    const gg = ge.createRadialGradient(x, y, 0, x, y, 6); gg.addColorStop(0, color); gg.addColorStop(1, 'rgba(0,0,0,0)');
    ge.fillStyle = gg; ge.beginPath(); ge.arc(x, y, 6, 0, Math.PI * 2); ge.fill();
    g.fillStyle = color; g.beginPath(); g.arc(x, y, 3.2, 0, Math.PI * 2); g.fill();
  };
  const keys = (x: number, n: number) => { for (let i = 0; i < n; i++) { roundRect(g, x + i * 26, hPx * 0.3, 20, hPx * 0.4, 3); g.fillStyle = i % 4 === 0 ? '#3a3d42' : '#2b2d31'; g.fill(); g.strokeStyle = '#111'; g.stroke(); } };
  const bays = (x: number, n: number) => { for (let i = 0; i < n; i++) { roundRect(g, x + i * 64, hPx * 0.14, 58, hPx * 0.72, 3); g.fillStyle = '#16171a'; g.fill(); g.fillStyle = '#3c3f44'; g.fillRect(x + i * 64 + 6, hPx * 0.5, 30, 3); led(x + i * 64 + 48, hPx * 0.3, i % 3 ? '#45f58a' : '#4aa3ff'); } };
  const brand = (e.brand ?? '').toUpperCase();
  g.font = `600 ${Math.round(Math.min(hPx * 0.2, 20))}px Helvetica, Arial`; g.textBaseline = 'top';
  g.fillStyle = alu ? '#2a2c30' : '#c8ccd2';
  // Sérigraphie : en haut à gauche sur 2 U et plus ; en bas à droite, discrète, sur 1 U.
  const tall = e.height_u >= 1.5;
  const mark = brand || e.label.split(' ')[0]!.toUpperCase();
  if (e.nature !== 'accessory' && tall) {
    g.fillText(mark, x0, hPx * 0.06);
    g.font = `400 ${Math.round(Math.min(hPx * 0.12, 15))}px Helvetica, Arial`; g.fillStyle = alu ? '#4a4d52' : '#8b9096';
    g.fillText(e.label.slice(0, 40), x0, hPx * 0.06 + Math.min(hPx * 0.17, 22));
  } else if (e.nature !== 'accessory') {
    g.font = `600 ${Math.round(hPx * 0.17)}px Helvetica, Arial`; g.textAlign = 'right'; g.textBaseline = 'bottom';
    g.fillText(mark.slice(0, 18), x1 - 6, hPx * 0.94); g.textAlign = 'left'; g.textBaseline = 'top';
  }
  // Repère de l'exemplaire, étiquette blanche.
  if (e.tag) { const tw = g.measureText(e.tag).width + 14; roundRect(g, x1 - tw, hPx * 0.08, tw, 18, 3); g.fillStyle = '#f4f4f0'; g.fill(); g.fillStyle = '#15171a'; g.font = '600 12px Courier, monospace'; g.fillText(e.tag, x1 - tw + 7, hPx * 0.08 + 3); }
  const mid = (x0 + x1) / 2, avail = x1 - x0;
  switch (t) {
    case 'ups': lcd(x0, 120, '230V', '#ffb02e'); vents(x0 + 150, x1 - 80); led(x1 - 30, hPx / 2, '#45f58a'); break;
    case 'router': lcd(x0, 140, '12G-SDI', '#6cf2c2'); keys(x0 + 170, Math.min(16, Math.floor((avail - 230) / 26))); led(x1 - 20, hPx / 2, '#45f58a'); break;
    case 'mixer': vents(x0, x0 + avail * 0.4); lcd(mid, 110, 'M/E 1', '#7cc8ff'); led(x1 - 20, hPx / 2, '#45f58a'); break;
    case 'replay': case 'server': bays(x0, Math.max(2, Math.floor(avail * 0.7 / 64))); led(x1 - 20, hPx / 2, '#45f58a'); if (t === 'replay') lcd(x1 - 150, 110, 'REC', '#ff5a4f'); break;
    case 'cards': for (let i = 0; i < Math.floor(avail / 34); i++) { roundRect(g, x0 + i * 34, hPx * 0.08, 28, hPx * 0.84, 2); g.fillStyle = '#1a1b1e'; g.fill(); g.fillStyle = '#b8bcc2'; g.fillRect(x0 + i * 34 + 10, hPx * 0.7, 8, 8); led(x0 + i * 34 + 14, hPx * 0.2, i % 5 ? '#45f58a' : '#ffb02e'); } break;
    case 'scope': { const w = Math.min(avail * 0.45, 260); roundRect(g, x0, hPx * 0.12, w, hPx * 0.76, 4); g.fillStyle = '#05080a'; g.fill(); ge.strokeStyle = '#5cf29a'; ge.lineWidth = 2; ge.beginPath(); for (let i = 0; i < 40; i++) ge.lineTo(x0 + 8 + i * (w - 16) / 40, hPx * (0.3 + 0.4 * r())); ge.stroke(); g.strokeStyle = 'rgba(92,242,154,.8)'; g.stroke(); keys(x0 + w + 20, 4); break; }
    case 'sync': lcd(x0, 120, 'PTP LOCK', '#6cf2c2'); led(x0 + 140, hPx / 2, '#45f58a'); led(x0 + 160, hPx / 2, '#45f58a'); break;
    case 'mv': lcd(x0, 90, 'MV', '#7cc8ff'); vents(x0 + 120, x1 - 40); break;
    case 'rec': bays(x0, 2); lcd(x0 + 150, 120, '00:42:17', '#ff5a4f'); break;
    case 'intercom': lcd(x0, 110, 'IC-1', '#ffb02e'); keys(x0 + 130, 8); break;
    case 'ccu': lcd(x0, 100, 'CAM 3', '#7cc8ff'); keys(x0 + 120, 6); break;
    case 'rx': lcd(x0, 140, '606.125', '#7cc8ff'); lcd(x0 + 170, 140, '614.450', '#7cc8ff'); break;
    case 'monitor': case 'monitor2': case 'monitor3': case 'monitor4': {
      // Écrans 16/9 sur presque toute la hauteur, côte à côte, chacun avec sa mire et son UMD.
      const n = t === 'monitor' ? 1 : Number(t.slice(7));
      const keysW = 30, room = avail - keysW, gap = room * 0.03;
      let hh = hPx * 0.8, w = hh * 16 / 9;
      if (n * w + (n - 1) * gap > room) { w = (room - (n - 1) * gap) / n; hh = w * 9 / 16; }
      const y = (hPx - hh) / 2, start = x0 + (room - (n * w + (n - 1) * gap)) / 2;
      const top = ['#bfbfbf', '#bfbf00', '#00bfbf', '#00bf00', '#bf00bf', '#bf0000', '#0000bf'];
      const mid2 = ['#0000bf', '#131313', '#bf00bf', '#131313', '#00bfbf', '#131313', '#bfbfbf'];
      for (let k = 0; k < n; k++) {
        const sx = start + k * (w + gap);
        roundRect(g, sx - 3, y - 3, w + 6, hh + 6, 3); g.fillStyle = '#0a0b0d'; g.fill();
        const paint = (ctx: CanvasRenderingContext2D, a: number) => {
          ctx.globalAlpha = a; const bw = w / 7;
          top.forEach((c, i) => { ctx.fillStyle = c; ctx.fillRect(sx + i * bw, y, bw + 0.5, hh * 0.6); });
          mid2.forEach((c, i) => { ctx.fillStyle = c; ctx.fillRect(sx + i * bw, y + hh * 0.6, bw + 0.5, hh * 0.08); });
          [['#00214c', 0, 0.18], ['#ffffff', 0.18, 0.36], ['#32006a', 0.36, 0.54], ['#131313', 0.54, 1]].forEach(([c, a1, a2]) => { ctx.fillStyle = c as string; ctx.fillRect(sx + w * (a1 as number), y + hh * 0.68, w * ((a2 as number) - (a1 as number)) + 0.5, hh * 0.2); });
          ctx.fillStyle = '#0b0c0e'; ctx.fillRect(sx, y + hh * 0.88, w, hh * 0.12);
          ctx.globalAlpha = 1;
        };
        paint(g, 1); paint(ge, 0.55);
        g.fillStyle = '#3bd16f'; g.fillRect(sx, y + hh * 0.88, 4, hh * 0.12);
      }
      for (let i = 0; i < 5; i++) { roundRect(g, x1 - 18, hPx * 0.2 + i * hPx * 0.13, 12, hPx * 0.08, 2); g.fillStyle = '#8f969e'; g.fill(); }
      break;
    }
    case 'vent': vents(x0, x1); break;
    case 'brush': { roundRect(g, x0, hPx * 0.3, avail, hPx * 0.4, 6); g.fillStyle = '#060607'; g.fill(); for (let i = 0; i < avail; i += 2) { g.strokeStyle = `rgba(30,30,32,${0.5 + r() * 0.5})`; g.beginPath(); g.moveTo(x0 + i, hPx * 0.3); g.lineTo(x0 + i + (r() - 0.5) * 3, hPx * 0.5); g.moveTo(x0 + i, hPx * 0.7); g.lineTo(x0 + i + (r() - 0.5) * 3, hPx * 0.5); g.stroke(); } break; }
    case 'drawer': { roundRect(g, mid - 80, hPx * 0.38, 160, hPx * 0.24, 6); g.fillStyle = '#9aa0a8'; g.fill(); break; }
    case 'pdu': for (let i = 0; i < Math.floor(avail / 46); i++) { roundRect(g, x0 + i * 46, hPx * 0.16, 38, hPx * 0.68, 4); g.fillStyle = '#0d0e10'; g.fill(); } led(x1 - 10, hPx / 2, '#ff5a4f'); break;
    case 'switch': case 'bnc': case 'xlr': case 'rjp': case 'stage': case 'dist': case 'blank': break;
    default: if (e.nature !== 'accessory') { vents(mid, x1 - 40); led(x1 - 20, hPx / 2, '#45f58a'); }
  }
  return { map: tex(c), emissive: tex(ce), rough: tex(cr, false) };
}

// ----- Connecteurs -----

type CKind = 'bnc' | 'xlr' | 'rj45' | 'fiber' | 'hdmi' | 'power' | 'generic';
function ckind(id: string): CKind {
  const s = id.toLowerCase();
  if (s.includes('bnc') || s.includes('din1.0') || s.includes('hd-bnc')) return 'bnc';
  if (s.includes('xlr')) return 'xlr';
  if (s.includes('rj45') || s.includes('ethercon')) return 'rj45';
  if (s.includes('lc') || s.includes('sc') || s.includes('opticalcon') || s.includes('fib')) return 'fiber';
  if (s.includes('hdmi') || s.includes('dvi') || s.includes('dp')) return 'hdmi';
  if (s.includes('iec') || s.includes('powercon') || s.includes('schuko') || s.includes('p17') || s.includes('true1')) return 'power';
  return 'generic';
}
const CSIZE: Record<CKind, [number, number]> = { bnc: [0.016, 0.016], xlr: [0.027, 0.032], rj45: [0.016, 0.014], fiber: [0.014, 0.012], hdmi: [0.02, 0.01], power: [0.03, 0.026], generic: [0.014, 0.014] };

interface Mats { [k: string]: THREE.Material }

/** Un connecteur, tourné vers +Z, centré sur l'origine de sa face. */
function connector(k: CKind, m: Mats, color: string | null): THREE.Object3D {
  const g = new THREE.Group();
  if (k === 'bnc') {
    const body = new THREE.Mesh(new THREE.LatheGeometry([new THREE.Vector2(0.0062, 0), new THREE.Vector2(0.0062, 0.0015), new THREE.Vector2(0.0047, 0.0018), new THREE.Vector2(0.0047, 0.011), new THREE.Vector2(0.0042, 0.0115), new THREE.Vector2(0.0042, 0.0125)], 24), m.nickel);
    body.rotation.x = Math.PI / 2; g.add(body);
    const ins = new THREE.Mesh(new THREE.CylinderGeometry(0.0033, 0.0033, 0.001, 20), m.ptfe); ins.rotation.x = Math.PI / 2; ins.position.z = 0.012; g.add(ins);
    const pin = new THREE.Mesh(new THREE.CylinderGeometry(0.0006, 0.0006, 0.002, 8), m.gold); pin.rotation.x = Math.PI / 2; pin.position.z = 0.0125; g.add(pin);
    for (const s of [-1, 1]) { const lug = new THREE.Mesh(new THREE.BoxGeometry(0.0012, 0.0012, 0.0016), m.nickel); lug.position.set(s * 0.0049, 0, 0.0075); g.add(lug); }
    if (color) { const ring = new THREE.Mesh(new THREE.TorusGeometry(0.0055, 0.0008, 8, 24), new THREE.MeshStandardMaterial({ color, roughness: 0.5 })); ring.position.z = 0.0016; g.add(ring); }
  } else if (k === 'xlr') {
    const fl = new THREE.Mesh(new RoundedBoxGeometry(0.026, 0.031, 0.0025, 2, 0.002), m.blackMetal); fl.position.z = 0.00125; g.add(fl);
    const cup = new THREE.Mesh(new THREE.CylinderGeometry(0.0105, 0.0105, 0.004, 32), m.blackPlastic); cup.rotation.x = Math.PI / 2; cup.position.z = 0.003; g.add(cup);
    const face = new THREE.Mesh(new THREE.CircleGeometry(0.0085, 32), m.xlrFace); face.position.z = 0.0051; g.add(face);
    const latch = new THREE.Mesh(new THREE.BoxGeometry(0.008, 0.004, 0.0015), m.nickel); latch.position.set(0, 0.0125, 0.0032); g.add(latch);
    for (const s of [-1, 1]) { const sc = new THREE.Mesh(new THREE.CylinderGeometry(0.0013, 0.0013, 0.0008, 10), m.nickel); sc.rotation.x = Math.PI / 2; sc.position.set(s * 0.0095, s * -0.012, 0.0028); g.add(sc); }
  } else if (k === 'rj45') {
    const frame = new THREE.Mesh(new THREE.BoxGeometry(0.0158, 0.0138, 0.003), m.nickel); frame.position.z = 0.0015; g.add(frame);
    const hole = new THREE.Mesh(new THREE.BoxGeometry(0.0118, 0.0098, 0.0005), m.cavity); hole.position.z = 0.0031; g.add(hole);
    for (const [x, c] of [[-0.0055, '#45f58a'], [0.0055, '#ffb02e']] as [number, string][]) { const l = new THREE.Mesh(new THREE.BoxGeometry(0.0022, 0.0016, 0.0004), new THREE.MeshStandardMaterial({ color: c, emissive: c, emissiveIntensity: 4 })); l.position.set(x, 0.0058, 0.0032); g.add(l); }
  } else if (k === 'fiber') {
    const b = new THREE.Mesh(new RoundedBoxGeometry(0.0135, 0.0105, 0.004, 2, 0.001), new THREE.MeshStandardMaterial({ color: color ?? '#2f7de1', roughness: 0.45 })); b.position.z = 0.002; g.add(b);
    for (const s of [-1, 1]) { const h = new THREE.Mesh(new THREE.BoxGeometry(0.0042, 0.0042, 0.0004), m.cavity); h.position.set(s * 0.0032, 0, 0.0041); g.add(h); }
  } else if (k === 'hdmi') {
    const s = new THREE.Shape(); s.moveTo(-0.0078, 0.0028); s.lineTo(0.0078, 0.0028); s.lineTo(0.0078, -0.0008); s.lineTo(0.0058, -0.0028); s.lineTo(-0.0058, -0.0028); s.lineTo(-0.0078, -0.0008); s.closePath();
    const sh = new THREE.Mesh(new THREE.ExtrudeGeometry(s, { depth: 0.002, bevelEnabled: false }), m.nickel); g.add(sh);
    const hole = new THREE.Mesh(new THREE.BoxGeometry(0.0128, 0.0036, 0.0004), m.cavity); hole.position.z = 0.0021; g.add(hole);
  } else if (k === 'power') {
    const b = new THREE.Mesh(new RoundedBoxGeometry(0.027, 0.022, 0.006, 2, 0.002), m.blackPlastic); b.position.z = 0.003; g.add(b);
    const f = new THREE.Mesh(new THREE.BoxGeometry(0.018, 0.012, 0.0005), m.cavity); f.position.z = 0.0061; g.add(f);
    for (const x of [-0.005, 0, 0.005]) { const p = new THREE.Mesh(new THREE.BoxGeometry(0.0012, 0.0045, 0.002), m.nickel); p.position.set(x, x ? -0.001 : 0.002, 0.006); g.add(p); }
  } else {
    const b = new THREE.Mesh(new THREE.CylinderGeometry(0.0055, 0.0055, 0.004, 20), m.nickel); b.rotation.x = Math.PI / 2; b.position.z = 0.002; g.add(b);
    const h = new THREE.Mesh(new THREE.CircleGeometry(0.0035, 20), m.cavity); h.position.z = 0.0041; g.add(h);
  }
  g.traverse((o) => { if ((o as THREE.Mesh).isMesh) { o.castShadow = false; o.receiveShadow = true; } });
  return g;
}

/** Dispose des connecteurs en grille sur une face (largeur w, hauteur h), vers +Z. */
function layoutConnectors(list: { k: CKind; color: string | null }[], w: number, h: number, m: Mats): THREE.Group {
  const g = new THREE.Group();
  if (!list.length) return g;
  const size = list.reduce((a, c) => [Math.max(a[0], CSIZE[c.k][0]), Math.max(a[1], CSIZE[c.k][1])], [0, 0]);
  const gap = 0.004;
  let cols = Math.max(1, Math.floor((w + gap) / (size[0] + gap)));
  let rows = Math.max(1, Math.floor((h + gap) / (size[1] + gap)));
  let scale = 1;
  while (cols * rows < list.length && scale > 0.4) { scale *= 0.88; cols = Math.max(1, Math.floor((w + gap) / (size[0] * scale + gap))); rows = Math.max(1, Math.floor((h + gap) / (size[1] * scale + gap))); }
  const n = Math.min(list.length, cols * rows);
  const usedRows = Math.ceil(n / cols), perRow = Math.min(cols, n);
  const sx = size[0] * scale + gap, sy = size[1] * scale + gap;
  for (let i = 0; i < n; i++) {
    const r = Math.floor(i / cols), c = i % cols;
    const o = connector(list[i]!.k, m, list[i]!.color); o.scale.setScalar(scale);
    o.position.set((c - (perRow - 1) / 2) * sx, ((usedRows - 1) / 2 - r) * sy, 0);
    g.add(o);
  }
  return g;
}

// ----- Scène -----

export class RackScene {
  renderer: THREE.WebGLRenderer; scene = new THREE.Scene(); camera: THREE.PerspectiveCamera; controls: OrbitControls;
  composer: EffectComposer; private raf = 0; private doors: { pivot: THREE.Object3D; target: number }[] = [];
  private sides: THREE.Object3D[] = []; private pick: THREE.Object3D[] = []; private ray = new THREE.Raycaster(); private mats: Mats;
  private root = new THREE.Group(); private dims = new THREE.Vector3(0.6, 2, 0.8); private disposed = false;
  onHover: (h: Hit | null, x: number, y: number) => void = () => {};

  constructor(private host: HTMLElement) {
    this.renderer = new THREE.WebGLRenderer({ antialias: true, preserveDrawingBuffer: true, alpha: false });
    this.renderer.setPixelRatio(Math.min(2, window.devicePixelRatio));
    this.renderer.toneMapping = THREE.ACESFilmicToneMapping; this.renderer.toneMappingExposure = 1.28;
    this.renderer.outputColorSpace = THREE.SRGBColorSpace;
    this.renderer.shadowMap.enabled = true; this.renderer.shadowMap.type = THREE.PCFSoftShadowMap;
    host.appendChild(this.renderer.domElement);
    this.camera = new THREE.PerspectiveCamera(32, 1, 0.02, 60);
    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.enableDamping = true; this.controls.dampingFactor = 0.08; this.controls.minDistance = 0.3; this.controls.maxDistance = 12; this.controls.maxPolarAngle = Math.PI * 0.495;
    const pmrem = new THREE.PMREMGenerator(this.renderer);
    this.scene.environment = pmrem.fromScene(new RoomEnvironment(), 0.04).texture;
    this.scene.environmentIntensity = 1.2;
    this.scene.background = new THREE.Color('#3a3e44');
    this.scene.fog = new THREE.Fog('#3a3e44', 7, 18);
    // Lumières : clé avec ombres douces, contre-jour froid.
    const key = new THREE.DirectionalLight('#fff4e6', 2.9); key.position.set(2.2, 4, 3.2); key.castShadow = true;
    key.shadow.mapSize.set(2048, 2048); key.shadow.camera.left = -2; key.shadow.camera.right = 2; key.shadow.camera.top = 3; key.shadow.camera.bottom = -1; key.shadow.bias = -0.0004; key.shadow.radius = 6; key.shadow.normalBias = 0.02;
    this.scene.add(key);
    const rim = new THREE.DirectionalLight('#b8d4ff', 1.1); rim.position.set(-3, 2.5, -2.5); this.scene.add(rim);
    this.scene.add(new THREE.HemisphereLight('#eef2ff', '#3a342e', 0.8));
    // Débouchage de face : on lit les façades même en vue de face.
    const fill = new THREE.DirectionalLight('#ffffff', 0.75); fill.position.set(0.4, 1.6, 5); this.scene.add(fill);
    // Sol : béton lissé, rugosité variable.
    const floorTex = tex(noise(512, 120, 40, 3), true, [8, 8]);
    const floorR = tex(noise(512, 170, 70, 5), false, [8, 8]);
    const floor = new THREE.Mesh(new THREE.CircleGeometry(9, 64), new THREE.MeshStandardMaterial({ color: '#6a6d72', map: floorTex, roughnessMap: floorR, roughness: 0.92, metalness: 0 }));
    floor.rotation.x = -Math.PI / 2; floor.receiveShadow = true; this.scene.add(floor);
    this.scene.add(this.root);
    this.mats = this.materials();
    this.composer = new EffectComposer(this.renderer);
    this.composer.addPass(new RenderPass(this.scene, this.camera));
    this.composer.addPass(new UnrealBloomPass(new THREE.Vector2(512, 512), 0.35, 0.35, 1.6));
    this.composer.addPass(new OutputPass());
    this.resize();
    this.renderer.domElement.addEventListener('pointermove', this.hover);
    this.renderer.domElement.addEventListener('pointerleave', () => this.onHover(null, 0, 0));
    this.loop();
  }

  private materials(): Mats {
    const brushed = tex(noise(256, 150, 20, 11, 40), true, [2, 1]);
    const brushedR = tex(noise(256, 90, 40, 12, 50), false, [2, 1]);
    const powder = tex(noise(256, 140, 50, 13), false, [4, 4]);
    const ham = hammered();
    const xlr = (() => { const [c, g] = canvas(128, 128); g.fillStyle = '#141416'; g.beginPath(); g.arc(64, 64, 64, 0, Math.PI * 2); g.fill(); g.fillStyle = '#000'; for (const [x, y] of [[40, 52], [88, 52], [64, 92]]) { g.beginPath(); g.arc(x!, y!, 11, 0, Math.PI * 2); g.fill(); } return tex(c); })();
    return {
      powder: new THREE.MeshStandardMaterial({ color: '#1c1d20', roughness: 0.62, metalness: 0.35, roughnessMap: powder }),
      steel: new THREE.MeshStandardMaterial({ color: '#2a2c30', roughness: 0.5, metalness: 0.6, roughnessMap: powder }),
      alu: new THREE.MeshPhysicalMaterial({ color: '#d4d7dc', metalness: 1, roughness: 0.32, map: brushed, roughnessMap: brushedR, anisotropy: 0.6, clearcoat: 0.1 }),
      chrome: new THREE.MeshStandardMaterial({ color: '#c9ccd1', metalness: 1, roughness: 0.2 }),
      extrusion: new THREE.MeshStandardMaterial({ color: '#c6c9ce', metalness: 1, roughness: 0.34 }),
      nickel: new THREE.MeshStandardMaterial({ color: '#d9dbdf', metalness: 1, roughness: 0.22 }),
      gold: new THREE.MeshStandardMaterial({ color: '#e7c06a', metalness: 1, roughness: 0.25 }),
      ptfe: new THREE.MeshStandardMaterial({ color: '#f3f1ea', roughness: 0.6 }),
      blackPlastic: new THREE.MeshStandardMaterial({ color: '#141416', roughness: 0.55, metalness: 0 }),
      blackMetal: new THREE.MeshStandardMaterial({ color: '#17181b', roughness: 0.4, metalness: 0.7 }),
      cavity: new THREE.MeshStandardMaterial({ color: '#030304', roughness: 0.9 }),
      xlrFace: new THREE.MeshStandardMaterial({ map: xlr, roughness: 0.6 }),
      rubber: new THREE.MeshStandardMaterial({ color: '#0c0c0d', roughness: 0.92 }),
      laminate: new THREE.MeshPhysicalMaterial({ color: '#2b2c2f', roughness: 0.62, metalness: 0.0, bumpMap: tex(ham, false, [3, 3]), bumpScale: 1.6, clearcoat: 0.25, clearcoatRoughness: 0.6 }),
      plywood: new THREE.MeshStandardMaterial({ color: '#b48a5a', roughness: 0.8, map: tex(noise(256, 150, 60, 21, 30), true, [1, 4]) }),
      foam: new THREE.MeshStandardMaterial({ color: '#18191b', roughness: 1 }),
      chassis: new THREE.MeshStandardMaterial({ color: '#16171a', roughness: 0.5, metalness: 0.6 }),
    };
  }

  /** Construit (ou reconstruit) l'élément et ses équipements. */
  build(spec: Box3DSpec, els: El3D[]) {
    this.root.clear(); this.doors = []; this.sides = []; this.pick = [];
    const m = this.mats;
    const units = Math.max(1, spec.units);
    const isCase = /case/.test(spec.kind);
    const waterproof = spec.kind === 'waterproof_case';
    const W = (spec.width_mm ?? (isCase ? 560 : 600)) / 1000;
    const D = (spec.depth_mm ?? (isCase ? 600 : 800)) / 1000;
    const base = isCase ? (spec.wheels ? 0.16 : 0.03) : spec.wheels ? 0.13 : 0.1;
    const top = isCase ? 0.06 : 0.12;
    const H = spec.height_mm ? spec.height_mm / 1000 : units * U + base + top;
    const y0 = base + (H - base - top - units * U) / 2 + (isCase ? 0.025 : 0); // bas de la U1
    const frontRail = D / 2 - (isCase ? 0.075 : 0.07);
    const rearRail = -D / 2 + (spec.rear_rails_mm ?? (isCase ? 75 : 70)) / 1000;
    this.dims.set(W, H, D);
    const add = (geo: THREE.BufferGeometry, mat: THREE.Material, x: number, y: number, z: number, parent: THREE.Object3D = this.root, shadow = true) => {
      const o = new THREE.Mesh(geo, mat); o.position.set(x, y, z); o.castShadow = shadow; o.receiveShadow = true; parent.add(o); return o;
    };

    // Montants 19" (avant et arrière), trous carrés et numérotation.
    const rt = railTextures(units);
    const railMat = new THREE.MeshStandardMaterial({ color: '#c9ccd1', metalness: 0.95, roughness: 0.38, map: rt.map, alphaMap: rt.alpha, alphaTest: 0.5, side: THREE.DoubleSide });
    for (const z of [frontRail, rearRail]) for (const s of [-1, 1]) {
      const rail = new THREE.Group(); rail.position.set(s * (OPEN / 2 + 0.008), y0 + (units * U) / 2, z);
      const flange = new THREE.Mesh(new THREE.PlaneGeometry(0.016, units * U), railMat); flange.rotation.y = 0; flange.position.z = z > 0 ? 0.0005 : -0.0005; if (z < 0) flange.rotation.y = Math.PI; flange.castShadow = true; rail.add(flange);
      const web = new THREE.Mesh(new THREE.BoxGeometry(0.0015, units * U, 0.045), m.steel); web.position.set(s * 0.008, 0, z > 0 ? -0.0225 : 0.0225); web.castShadow = true; rail.add(web);
      this.root.add(rail);
    }

    if (!isCase) {
      // Baie : ossature, panneaux, toit ajouré, socle, roulettes ou vérins, portes.
      const post = 0.04;
      for (const sx of [-1, 1]) for (const sz of [-1, 1]) add(new THREE.BoxGeometry(post, H - base, post), m.powder, sx * (W / 2 - post / 2), base + (H - base) / 2, sz * (D / 2 - post / 2));
      for (const y of [base + 0.02, H - 0.02]) for (const sz of [-1, 1]) add(new THREE.BoxGeometry(W, 0.04, post), m.powder, 0, y, sz * (D / 2 - post / 2));
      for (const y of [base + 0.02, H - 0.02]) for (const sx of [-1, 1]) add(new THREE.BoxGeometry(post, 0.04, D), m.powder, sx * (W / 2 - post / 2), y, 0);
      // Toit avec ouïes.
      const roof = new THREE.Group(); roof.position.y = H;
      add(new THREE.BoxGeometry(W, 0.012, D), m.powder, 0, 0.006, 0, roof);
      for (let i = 0; i < 6; i++) add(new THREE.BoxGeometry(W * 0.55, 0.004, 0.012), m.cavity, 0, 0.0125, -D / 4 + i * (D / 2) / 5, roof, false);
      this.root.add(roof);
      // Socle.
      add(new THREE.BoxGeometry(W - 0.01, base - (spec.wheels ? 0.06 : 0.015), D - 0.01), m.powder, 0, (base + (spec.wheels ? 0.06 : 0.015)) / 2, 0);
      for (const sx of [-1, 1]) for (const sz of [-1, 1]) {
        if (spec.wheels) this.castor(sx * (W / 2 - 0.06), sz * (D / 2 - 0.06), 0.06);
        else { add(new THREE.CylinderGeometry(0.018, 0.022, 0.012, 20), m.rubber, sx * (W / 2 - 0.05), 0.006, sz * (D / 2 - 0.05)); add(new THREE.CylinderGeometry(0.006, 0.006, 0.02, 12), m.nickel, sx * (W / 2 - 0.05), 0.02, sz * (D / 2 - 0.05)); }
      }
      // Panneaux latéraux (masquables).
      for (const sx of [-1, 1]) { const p = add(new THREE.BoxGeometry(0.008, H - base - 0.02, D - 0.08), m.powder, sx * (W / 2 - 0.004), base + (H - base) / 2, 0); this.sides.push(p); }
      // Portes perforées, gond à gauche (avant) et à droite (arrière).
      const perf = tex(perforation(), false, [Math.round(W / 0.011), Math.round(H / 0.019)]);
      const doorMat = new THREE.MeshStandardMaterial({ color: '#18191c', metalness: 0.5, roughness: 0.55, alphaMap: perf, alphaTest: 0.5, side: THREE.DoubleSide });
      for (const front of [true, false]) {
        const pivot = new THREE.Group(); const hx = front ? -W / 2 + 0.01 : W / 2 - 0.01; pivot.position.set(hx, base + (H - base) / 2, front ? D / 2 + 0.006 : -D / 2 - 0.006);
        const dw = W - 0.02, dh = H - base - 0.04, dir = front ? 1 : -1;
        const leaf = new THREE.Mesh(new THREE.PlaneGeometry(dw - 0.06, dh - 0.06), doorMat); leaf.position.x = dir * dw / 2; if (!front) leaf.rotation.y = Math.PI; leaf.castShadow = true; pivot.add(leaf);
        const fr = new THREE.MeshStandardMaterial({ color: '#1a1b1e', metalness: 0.45, roughness: 0.5 });
        for (const [x, y, w, h] of [[dw / 2, dh / 2 - 0.015, dw, 0.03], [dw / 2, -dh / 2 + 0.015, dw, 0.03], [0.015, 0, 0.03, dh], [dw - 0.015, 0, 0.03, dh]] as number[][]) { const b = new THREE.Mesh(new THREE.BoxGeometry(w!, h!, 0.012), fr); b.position.set(dir * x!, y!, 0); b.castShadow = true; pivot.add(b); }
        const handle = new THREE.Mesh(new RoundedBoxGeometry(0.018, 0.16, 0.022, 2, 0.006), m.chrome); handle.position.set(dir * (dw - 0.04), 0.1, dir * 0.014); pivot.add(handle);
        this.root.add(pivot); this.doors.push({ pivot, target: 0 });
      }
    } else {
      // Flight-case : peau stratifiée sur contreplaqué, cornières alu, coins boule, roulettes.
      const t = 0.009; const shell = waterproof ? new THREE.MeshPhysicalMaterial({ color: '#24272b', roughness: 0.55, clearcoat: 0.3, clearcoatRoughness: 0.5 }) : m.laminate;
      const yb = base, hh = H - base;
      const panel = (w: number, h: number, d: number, x: number, y: number, z: number) => add(waterproof ? new RoundedBoxGeometry(w, h, d, 3, 0.008) : new THREE.BoxGeometry(w, h, d), shell, x, y, z);
      panel(W, t, D, 0, yb + t / 2, 0); panel(W, t, D, 0, yb + hh - t / 2, 0);
      for (const sx of [-1, 1]) this.sides.push(panel(t, hh, D, sx * (W / 2 - t / 2), yb + hh / 2, 0));
      // Tranche de contreplaqué visible à l'ouverture.
      for (const z of [D / 2 - 0.001, -D / 2 + 0.001]) for (const [w, h, x, y] of [[W, t, 0, yb + t / 2], [W, t, 0, yb + hh - t / 2], [t, hh, -W / 2 + t / 2, yb + hh / 2], [t, hh, W / 2 - t / 2, yb + hh / 2]] as number[][]) add(new THREE.BoxGeometry(w!, h!, 0.002), m.plywood, x!, y!, z, this.root, false);
      if (!waterproof) {
        // Cornières en L sur les 12 arêtes.
        const L = (len: number) => { const s = new THREE.Shape(); const a = 0.03, e = 0.0022; s.moveTo(0, 0); s.lineTo(a, 0); s.lineTo(a, e); s.lineTo(e, e); s.lineTo(e, a); s.lineTo(0, a); s.closePath(); return new THREE.ExtrudeGeometry(s, { depth: len, bevelEnabled: true, bevelSize: 0.0006, bevelThickness: 0.0006, bevelSegments: 2 }); };
        const xs = [-W / 2, W / 2], ys = [yb, yb + hh], zs = [-D / 2, D / 2];
        for (const x of xs) for (const y of ys) { const me = new THREE.Mesh(L(D - 0.06), m.extrusion); me.position.set(x, y, -D / 2 + 0.03); me.scale.set(x < 0 ? 1 : -1, y === yb ? 1 : -1, 1); me.castShadow = true; this.root.add(me); }
        for (const z of zs) for (const y of ys) { const me = new THREE.Mesh(L(W - 0.06), m.extrusion); me.rotation.y = Math.PI / 2; me.position.set(-W / 2 + 0.03, y, z); me.scale.set(z < 0 ? -1 : 1, y === yb ? 1 : -1, 1); me.castShadow = true; this.root.add(me); }
        for (const x of xs) for (const z of zs) { const me = new THREE.Mesh(L(hh - 0.06), m.extrusion); me.rotation.x = -Math.PI / 2; me.position.set(x, yb + 0.03, z); me.scale.set(x < 0 ? 1 : -1, z < 0 ? -1 : 1, 1); me.castShadow = true; this.root.add(me); }
        // Coins boule chromés.
        for (const x of xs) for (const y of ys) for (const z of zs) {
          const ball = new THREE.Mesh(new THREE.SphereGeometry(0.019, 32, 20), m.chrome);
          ball.position.set(x - Math.sign(x) * 0.008, y + (y === yb ? 0.008 : -0.008), z - Math.sign(z) * 0.008); ball.castShadow = true; this.root.add(ball);
        }
        // Profil de recouvrement (lèvre) autour des ouvertures.
        for (const z of zs) for (const [w, h, x, y] of [[W - 0.02, 0.012, 0, yb + 0.006], [W - 0.02, 0.012, 0, yb + hh - 0.006], [0.012, hh - 0.02, -W / 2 + 0.006, yb + hh / 2], [0.012, hh - 0.02, W / 2 - 0.006, yb + hh / 2]] as number[][]) add(new THREE.BoxGeometry(w!, h!, 0.016), m.extrusion, x!, y!, z - Math.sign(z) * 0.008);
      }
      // Fermetures papillon et poignées encastrées sur les côtés.
      for (const sx of [-1, 1]) {
        for (const z of [D / 2 - 0.07, -D / 2 + 0.07]) for (const y of [yb + hh * 0.3, yb + hh * 0.7]) this.latch(sx * (W / 2 + 0.001), y, z, sx);
        this.handle(sx * (W / 2 + 0.001), yb + hh * 0.62, 0, sx);
      }
      if (spec.wheels) {
        add(new THREE.BoxGeometry(W, 0.02, D), m.plywood, 0, base - 0.01, 0);
        for (const sx of [-1, 1]) for (const sz of [-1, 1]) this.castor(sx * (W / 2 - 0.07), sz * (D / 2 - 0.08), base - 0.02);
      }
    }

    // Équipements : au rack, ou posés sur une étagère.
    for (const e of els.filter((x) => !x.shelf_id)) this.equipment(e, frontRail, rearRail, y0);
    const byShelf = new Map<string, El3D[]>();
    for (const e of els.filter((x) => x.shelf_id)) byShelf.set(e.shelf_id!, [...(byShelf.get(e.shelf_id!) ?? []), e]);
    for (const [sid, items] of byShelf) {
      const sh = els.find((x) => x.id === sid); if (!sh) continue;
      const ys = y0 + (sh.u_start - 1) * U + 0.003;
      const rear = sh.face !== 'front', z = rear ? rearRail : frontRail;
      const slot = (OPEN - 0.02) / items.length;
      items.forEach((it, i) => this.loose(it, -OPEN / 2 + 0.01 + slot * (i + 0.5), ys, z + (rear ? 0.15 : -0.15), slot, sh.height_u, rear));
    }
    this.frame('quarter');
  }

  /** Appareil hors format posé sur une étagère : moniteur (écran et mire) ou boîtier. */
  private loose(e: El3D, x: number, y: number, z: number, slot: number, shelfU: number, rear: boolean) {
    const m = this.mats; const g = new THREE.Group(); g.position.set(x, y, z); if (rear) g.rotation.y = Math.PI;
    const hit = { label: e.label, tag: e.tag, u: e.u_start, height: e.height_u, face: e.face } as Hit;
    if (/moniteur|écran|monitor/i.test(e.label)) {
      const w = Math.min(slot - 0.01, 0.38), h = Math.min(w * 0.6, Math.max(0.09, shelfU * U * 4));
      const body = new THREE.Mesh(new RoundedBoxGeometry(w, h, 0.045, 3, 0.006), m.blackPlastic); body.position.set(0, h / 2 + 0.03, 0); body.castShadow = true; g.add(body);
      // Mire de barres couleur (SMPTE), lumineuse.
      const [c, gc] = canvas(512, 300);
      const bars = ['#c0c0c0', '#c0c000', '#00c0c0', '#00c000', '#c000c0', '#c00000', '#0000c0'];
      bars.forEach((b, i) => { gc.fillStyle = b; gc.fillRect((i * 512) / 7, 0, 512 / 7 + 1, 200); });
      ['#0000c0', '#131313', '#c000c0', '#131313', '#00c0c0', '#131313', '#c0c0c0'].forEach((b, i) => { gc.fillStyle = b; gc.fillRect((i * 512) / 7, 200, 512 / 7 + 1, 25); });
      gc.fillStyle = '#00214c'; gc.fillRect(0, 225, 90, 75); gc.fillStyle = '#ffffff'; gc.fillRect(90, 225, 90, 75); gc.fillStyle = '#32006a'; gc.fillRect(180, 225, 90, 75); gc.fillStyle = '#131313'; gc.fillRect(270, 225, 242, 75);
      gc.fillStyle = 'rgba(0,0,0,.6)'; gc.fillRect(8, 8, 120, 26); gc.fillStyle = '#fff'; gc.font = 'bold 18px Helvetica'; gc.fillText(e.tag ?? 'PGM', 14, 28);
      const t = tex(c);
      const scr = new THREE.Mesh(new THREE.PlaneGeometry(w - 0.016, h - 0.022), new THREE.MeshPhysicalMaterial({ map: t, emissiveMap: t, emissive: '#ffffff', emissiveIntensity: 0.22, roughness: 0.35, clearcoat: 0.6, clearcoatRoughness: 0.05 }));
      scr.position.set(0, h / 2 + 0.03 + 0.003, 0.0231); scr.userData.hit = hit; this.pick.push(scr); g.add(scr);
      const foot = new THREE.Mesh(new THREE.BoxGeometry(w * 0.4, 0.03, 0.12), m.blackPlastic); foot.position.set(0, 0.015, -0.02); foot.castShadow = true; g.add(foot);
      const led = new THREE.Mesh(new THREE.BoxGeometry(0.003, 0.002, 0.001), new THREE.MeshStandardMaterial({ color: '#45f58a', emissive: '#45f58a', emissiveIntensity: 1.5 })); led.position.set(w / 2 - 0.012, 0.035, 0.0232); g.add(led);
    } else {
      const w = Math.min(slot - 0.01, 0.3), h = Math.max(0.04, shelfU * U * 0.7);
      const body = new THREE.Mesh(new RoundedBoxGeometry(w, h, 0.22, 3, 0.006), m.chassis); body.position.y = h / 2; body.castShadow = true; g.add(body);
      const f = faceCanvases({ ...e, width: 'half' }, Math.round(w * 2400), Math.round(h * 2400));
      const front = new THREE.Mesh(new THREE.PlaneGeometry(w - 0.006, h - 0.006), new THREE.MeshStandardMaterial({ map: f.map, emissiveMap: f.emissive, emissive: '#fff', emissiveIntensity: 3.2, roughnessMap: f.rough, metalness: 0.5 }));
      front.position.set(0, h / 2, 0.111); front.userData.hit = hit; this.pick.push(front); g.add(front);
    }
    this.root.add(g);
  }

  private castor(x: number, z: number, h: number) {
    const m = this.mats; const g = new THREE.Group(); g.position.set(x, 0, z);
    const plate = new THREE.Mesh(new THREE.BoxGeometry(0.07, 0.006, 0.07), m.nickel); plate.position.y = h - 0.003; g.add(plate);
    const fork = new THREE.Mesh(new THREE.BoxGeometry(0.05, h - 0.03, 0.006), m.nickel); fork.position.set(0, (h - 0.03) / 2 + 0.025, 0.02); g.add(fork);
    const fork2 = fork.clone(); fork2.position.z = -0.02; g.add(fork2);
    const wheel = new THREE.Mesh(new THREE.CylinderGeometry(h * 0.45, h * 0.45, 0.032, 32), m.rubber); wheel.rotation.x = Math.PI / 2; wheel.position.y = h * 0.45; g.add(wheel);
    const hub = new THREE.Mesh(new THREE.CylinderGeometry(h * 0.2, h * 0.2, 0.034, 20), new THREE.MeshStandardMaterial({ color: '#c4342d', roughness: 0.4 })); hub.rotation.x = Math.PI / 2; hub.position.y = h * 0.45; g.add(hub);
    g.traverse((o) => { o.castShadow = true; });
    this.root.add(g);
  }

  private latch(x: number, y: number, z: number, side: number) {
    const m = this.mats; const g = new THREE.Group(); g.position.set(x, y, z); g.rotation.y = side * Math.PI / 2;
    const dish = new THREE.Mesh(new RoundedBoxGeometry(0.075, 0.075, 0.006, 3, 0.012), m.chrome); g.add(dish);
    const well = new THREE.Mesh(new THREE.CylinderGeometry(0.022, 0.024, 0.004, 32), m.cavity); well.rotation.x = Math.PI / 2; well.position.z = 0.0015; g.add(well);
    const s = new THREE.Shape(); s.moveTo(-0.022, -0.006); s.quadraticCurveTo(0, -0.012, 0.022, -0.006); s.lineTo(0.022, 0.006); s.quadraticCurveTo(0, 0.012, -0.022, 0.006); s.closePath();
    const wing = new THREE.Mesh(new THREE.ExtrudeGeometry(s, { depth: 0.004, bevelEnabled: true, bevelSize: 0.0012, bevelThickness: 0.001 }), m.chrome); wing.position.z = 0.003; g.add(wing);
    g.traverse((o) => { o.castShadow = true; o.receiveShadow = true; });
    this.root.add(g);
  }

  private handle(x: number, y: number, z: number, side: number) {
    const m = this.mats; const g = new THREE.Group(); g.position.set(x, y, z); g.rotation.y = side * Math.PI / 2;
    const dish = new THREE.Mesh(new RoundedBoxGeometry(0.2, 0.11, 0.008, 3, 0.012), m.chrome); g.add(dish);
    const well = new THREE.Mesh(new RoundedBoxGeometry(0.16, 0.07, 0.004, 3, 0.01), m.cavity); well.position.z = 0.003; g.add(well);
    const bar = new THREE.Mesh(new THREE.TorusGeometry(0.065, 0.007, 12, 32, Math.PI), m.blackPlastic); bar.rotation.z = Math.PI; bar.position.set(0, 0.03, 0.006); bar.rotation.x = -1.35; g.add(bar);
    g.traverse((o) => { o.castShadow = true; o.receiveShadow = true; });
    this.root.add(g);
  }

  private equipment(e: El3D, frontRail: number, rearRail: number, y0: number) {
    const m = this.mats;
    const n = { full: 1, half: 2, third: 3, quarter: 4 }[e.width] ?? 1;
    const fw = e.width === 'full' ? W19 : OPEN / n - 0.002;
    const cx = e.width === 'full' ? 0 : -OPEN / 2 + (e.col + 0.5) * (OPEN / n);
    const h = e.height_u * U - 0.0008;
    const y = y0 + (e.u_start - 1) * U + h / 2 + 0.0004;
    const rear = e.face !== 'front';
    const zFace = rear ? rearRail : frontRail;
    const dir = rear ? -1 : 1;
    const g = new THREE.Group(); g.position.set(cx, y, zFace + dir * 0.0015); if (rear) g.rotation.y = Math.PI;
    const depth = Math.max(0.02, Math.min((e.depth_mm ?? (e.nature === 'accessory' ? 30 : 350)) / 1000, Math.abs(frontRail - rearRail) + 0.12));
    const shelf = e.accessory?.kind === 'shelf';
    if (e.accessory?.kind === 'lacing_bar') {
      // Barre de laçage : deux équerres fixées au rail et une barre ronde déportée.
      const off = Math.max(0.03, (e.accessory.depth_mm || 100) / 1000);
      for (const s of [-1, 1]) {
        const ear = new THREE.Mesh(new THREE.BoxGeometry(0.018, h * 0.8, 0.002), m.blackMetal); ear.position.set(s * (W19 / 2 - 0.01), 0, 0); ear.castShadow = true; g.add(ear);
        const arm = new THREE.Mesh(new THREE.BoxGeometry(0.003, h * 0.5, off), m.blackMetal); arm.position.set(s * (OPEN / 2 - 0.004), 0, off / 2); arm.castShadow = true; g.add(arm);
      }
      const rod = new THREE.Mesh(new THREE.CylinderGeometry(0.0045, 0.0045, OPEN - 0.006, 16), m.chrome); rod.rotation.z = Math.PI / 2; rod.position.set(0, 0, off); rod.castShadow = true; g.add(rod);
      const hit = new THREE.Mesh(new THREE.BoxGeometry(OPEN, h, off), new THREE.MeshBasicMaterial({ visible: false })); hit.position.z = off / 2;
      hit.userData.hit = { label: e.label, tag: e.tag, u: e.u_start, height: e.height_u, face: e.face } as Hit; this.pick.push(hit); g.add(hit);
      this.root.add(g);
      return;
    }
    if (shelf) {
      const tray = new THREE.Mesh(new THREE.BoxGeometry(OPEN - 0.004, 0.002, depth), m.powder); tray.position.set(0, -h / 2 + 0.001, -depth / 2); tray.castShadow = tray.receiveShadow = true; g.add(tray);
      for (const s of [-1, 1]) { const lip = new THREE.Mesh(new THREE.BoxGeometry(0.002, 0.02, depth), m.powder); lip.position.set(s * (OPEN / 2 - 0.003), -h / 2 + 0.01, -depth / 2); g.add(lip); }
    } else if (e.nature !== 'accessory' || e.accessory?.kind === 'drawer') {
      // Châssis derrière la face.
      const body = new THREE.Mesh(new THREE.BoxGeometry(e.width === 'full' ? OPEN - 0.004 : fw - 0.004, h - 0.002, depth), m.chassis); body.position.z = -depth / 2 - 0.002; body.castShadow = true; body.receiveShadow = true; g.add(body);
    }
    // Face avant : plaque avec texture procédurale.
    const wPx = Math.round(fw * 2400), hPx = Math.max(40, Math.round(h * 2400));
    const f = faceCanvases(e, wPx, hPx);
    const faceMat = new THREE.MeshPhysicalMaterial({ map: f.map, emissiveMap: f.emissive, emissive: '#ffffff', emissiveIntensity: 3.2, roughnessMap: f.rough, roughness: e.alu ? 0.38 : 0.55, metalness: e.alu ? 0.9 : 0.55, clearcoat: e.alu ? 0.05 : 0.15 });
    // Plaque arrondie, et la face imprimée juste devant.
    const plate = new THREE.Mesh(new RoundedBoxGeometry(fw, h, 0.003, 2, 0.0008), m.chassis); plate.castShadow = true; plate.receiveShadow = true; g.add(plate);
    const front = new THREE.Mesh(new THREE.PlaneGeometry(fw - 0.0006, h - 0.0006), faceMat); front.position.z = 0.0016; front.receiveShadow = true; g.add(front);
    front.userData.hit = { label: e.label, tag: e.tag, u: e.u_start, height: e.height_u, face: e.face } as Hit; this.pick.push(front);
    // Poignées de face sur les appareils de 2 U et plus.
    if (e.nature !== 'accessory' && e.height_u >= 2 && e.width === 'full') for (const s of [-1, 1]) { const hd = new THREE.Mesh(new RoundedBoxGeometry(0.012, h * 0.7, 0.022, 2, 0.004), e.alu ? m.alu : m.blackMetal); hd.position.set(s * (W19 / 2 - 0.028), 0, 0.012); hd.castShadow = true; g.add(hd); }
    // Connecteurs : face avant ici, face arrière au dos du châssis.
    const conns = (faceKey: string) => {
      const list: { k: CKind; color: string | null }[] = [];
      for (const p of e.ports.filter((p) => (p.face || 'rear') === faceKey)) for (let i = 0; i < Math.min(p.quantity, 128); i++) list.push({ k: ckind(p.connector), color: p.color });
      return list;
    };
    if (e.accessory?.kind === 'panel') {
      // Panel : un emplacement par connecteur, en rangées par U.
      const per = e.accessory.panel.per_u as number, rows = e.accessory.panel.height_u as number;
      const sw = (OPEN - 0.02) / per;
      for (const s of e.accessory.panel.slots as any[]) {
        const idx = s.index - 1, r = Math.floor(idx / per), c = idx % per;
        const o = connector(ckind(s.connector), m, e.fnColor); const k = Math.min(1, (sw * 0.9) / CSIZE[ckind(s.connector)][0]); o.scale.setScalar(k);
        o.position.set(-OPEN / 2 + 0.01 + (c + (s.width ?? 1) / 2) * sw, (rows - 1) / 2 * U - r * U, 0.0018); g.add(o);
      }
    } else {
      const fl = conns('front');
      if (fl.length) { const cg = layoutConnectors(fl, Math.min(fw * 0.5, 0.24), h * 0.78, m); cg.position.set(fw * 0.18, 0, 0.0018); g.add(cg); }
      const rl = conns('rear');
      if (rl.length && e.nature !== 'accessory') {
        const back = new THREE.Mesh(new THREE.PlaneGeometry(fw - 0.01, h - 0.004), m.blackMetal); back.rotation.y = Math.PI; back.position.z = -depth - 0.0025; g.add(back);
        const cg = layoutConnectors(rl, fw - 0.04, h * 0.8, m); cg.rotation.y = Math.PI; cg.position.z = -depth - 0.003; g.add(cg);
      }
    }
    this.root.add(g);
  }

  /** Cadrages prédéfinis. */
  frame(v: 'front' | 'rear' | 'quarter' | 'side' | 'top') {
    const { x: W, y: H, z: D } = this.dims;
    // Distance qui fait tenir l'élément entier, marge comprise, selon le cadrage et l'écran.
    const vf = Math.tan(THREE.MathUtils.degToRad(this.camera.fov / 2));
    const span = v === 'side' ? D : v === 'quarter' ? Math.hypot(W, D) : W;
    const dist = Math.max((H * 0.62) / vf, (span * 0.66) / (vf * this.camera.aspect)) + (v === 'side' ? W / 2 : D / 2) + 0.15;
    const t = new THREE.Vector3(0, H * 0.48, 0);
    const p = { front: [0, H * 0.55, dist], rear: [0, H * 0.55, -dist], quarter: [dist * 0.62, H * 0.62 + dist * 0.18, dist * 0.78], side: [dist, H * 0.55, 0], top: [0.01, H + dist, 0.35] }[v];
    this.camera.position.set(p[0]!, p[1]!, p[2]!); this.controls.target.copy(t); this.controls.update();
  }
  /** Portes : fermées, ouvertes, ou retirées. */
  setDoors(mode: boolean | 'closed' | 'open' | 'removed') {
    const m = mode === true ? 'open' : mode === false ? 'closed' : mode;
    this.doors.forEach((d, i) => { d.pivot.visible = m !== 'removed'; d.target = m === 'open' ? (i === 0 ? -1.9 : 1.9) : 0; });
  }
  setSides(visible: boolean) { this.sides.forEach((s) => (s.visible = visible)); }

  private hover = (ev: PointerEvent) => {
    const r = this.renderer.domElement.getBoundingClientRect();
    const v = new THREE.Vector2(((ev.clientX - r.left) / r.width) * 2 - 1, -((ev.clientY - r.top) / r.height) * 2 + 1);
    this.ray.setFromCamera(v, this.camera);
    const hit = this.ray.intersectObjects(this.pick, false)[0];
    this.onHover(hit ? (hit.object.userData.hit as Hit) : null, ev.clientX - r.left, ev.clientY - r.top);
  };

  resize() {
    const w = Math.max(10, this.host.clientWidth), h = Math.max(10, this.host.clientHeight);
    this.renderer.setSize(w, h, false); this.composer.setSize(w, h); this.camera.aspect = w / h; this.camera.updateProjectionMatrix();
  }

  private loop = () => {
    if (this.disposed) return;
    this.raf = requestAnimationFrame(this.loop);
    for (const d of this.doors) d.pivot.rotation.y += (d.target - d.pivot.rotation.y) * 0.08;
    this.controls.update();
    this.composer.render();
  };

  snapshot(): string { this.composer.render(); return this.renderer.domElement.toDataURL('image/png'); }

  dispose() {
    this.disposed = true; cancelAnimationFrame(this.raf);
    this.renderer.domElement.removeEventListener('pointermove', this.hover);
    this.scene.traverse((o) => { const me = o as THREE.Mesh; if (me.isMesh) { me.geometry.dispose(); const ms = Array.isArray(me.material) ? me.material : [me.material]; ms.forEach((x) => { Object.values(x).forEach((v) => { if (v instanceof THREE.Texture) v.dispose(); }); x.dispose(); }); } });
    this.controls.dispose(); this.composer.dispose(); this.renderer.dispose(); this.renderer.domElement.remove();
  }
}
