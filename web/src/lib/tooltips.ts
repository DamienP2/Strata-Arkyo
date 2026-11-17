// Infobulles dessinées du design commun Strata (§ 4.21) : au survol à la souris de l'élément le plus
// proche portant un `title`, celui-ci passe dans `data-tip` (le navigateur n'affiche plus le sien ;
// aria-label est posé si l'élément n'a pas de texte), puis la carte .tip apparaît après 380 ms, à
// +14 / +18 px du pointeur, le suit et se retourne au bord de la fenêtre (marge 8). Elle disparaît
// quand le pointeur quitte l'élément, et sur pointerdown, molette, touche ou perte du focus.

let installed = false;
export function installTooltips() {
  if (installed || typeof document === 'undefined') return;
  installed = true;
  const tip = document.createElement('div');
  tip.id = 'tip';
  tip.className = 'tip';
  tip.setAttribute('role', 'tooltip');
  document.body.appendChild(tip);
  let el: Element | null = null;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let x = 0, y = 0;

  const hide = () => {
    if (timer) clearTimeout(timer);
    timer = null;
    el = null;
    tip.classList.remove('on');
  };
  const move = () => {
    const m = 8, w = tip.offsetWidth, h = tip.offsetHeight;
    let left = x + 14, top = y + 18;
    if (left + w > innerWidth - m) left = Math.max(m, x - 14 - w);
    if (top + h > innerHeight - m) top = Math.max(m, y - 14 - h);
    tip.style.transform = `translate(${Math.round(left)}px, ${Math.round(top)}px)`;
  };
  const target = (t: EventTarget | null): Element | null => {
    const e = t instanceof Element ? t.closest('[title], [data-tip]') : null;
    if (!e || e.closest('.tip')) return null;
    // Les <svg> et leurs <title> internes ne sont pas concernés : seul l'attribut compte.
    const title = e.getAttribute('title');
    if (title != null) {
      if (title.trim()) {
        e.setAttribute('data-tip', title);
        if (!e.getAttribute('aria-label') && !(e.textContent ?? '').trim()) e.setAttribute('aria-label', title);
      }
      e.removeAttribute('title');
    }
    return e.getAttribute('data-tip') ? e : null;
  };

  document.addEventListener('pointerover', (ev) => {
    if (ev.pointerType && ev.pointerType !== 'mouse') return;
    const e = target(ev.target);
    if (e === el) return;
    hide();
    if (!e) return;
    el = e;
    x = ev.clientX; y = ev.clientY;
    timer = setTimeout(() => {
      if (!el || !el.isConnected) return;
      tip.textContent = el.getAttribute('data-tip') ?? '';
      if (!tip.textContent) return;
      move();
      tip.classList.add('on');
    }, 380);
  }, true);
  document.addEventListener('pointermove', (ev) => {
    if (!el) return;
    x = ev.clientX; y = ev.clientY;
    if (tip.classList.contains('on')) move();
  }, true);
  document.addEventListener('pointerout', (ev) => {
    if (!el) return;
    const to = ev.relatedTarget as Node | null;
    if (!to || !el.contains(to)) hide();
  }, true);
  for (const t of ['pointerdown', 'keydown']) document.addEventListener(t, hide, true);
  addEventListener('wheel', hide, { capture: true, passive: true });
  addEventListener('blur', hide);
}
