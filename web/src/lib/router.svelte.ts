// Navigation par l'historique du navigateur : chaque écran a une adresse propre,
// que le moteur sert toujours avec la même page (application à page unique).

export const route = $state({ path: location.pathname, query: new URLSearchParams(location.search) });

function sync() {
  route.path = location.pathname;
  route.query = new URLSearchParams(location.search);
}

/** Écrans avec des modifications non enregistrées : on demande avant d'en partir. */
const guards = new Set<() => boolean>();
export const leave = $state<{ to: string | null }>({ to: null });
export function registerLeaveGuard(dirty: () => boolean): () => void { guards.add(dirty); return () => guards.delete(dirty); }
const blocked = () => [...guards].some((g) => g());
/** Quitter quand même (choix fait dans la fenêtre de confirmation). */
export function leaveAnyway() { const to = leave.to; leave.to = null; if (to) { guards.clear(); navigate(to); } }

export function navigate(to: string, replace = false) {
  if (to === location.pathname + location.search) return;
  if (blocked()) { leave.to = to; return; }
  if (replace) history.replaceState(null, '', to);
  else history.pushState(null, '', to);
  sync();
  // La page défile dans <main id="view"> (la coque reste fixe).
  document.getElementById('view')?.scrollTo(0, 0);
}

addEventListener('popstate', () => {
  // Retour arrière du navigateur avec des modifications en cours : on revient où l'on était
  // et on demande.
  const to = location.pathname + location.search;
  if (blocked() && to !== route.path + (route.query.toString() ? `?${route.query}` : '')) {
    history.pushState(null, '', route.path + (route.query.toString() ? `?${route.query}` : ''));
    leave.to = to; return;
  }
  sync();
});

/** Intercepte les clics sur les liens internes (<a href="/…">) pour ne pas recharger la page. */
addEventListener('click', (e) => {
  if (e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
  const a = (e.target as Element).closest?.('a[href]') as HTMLAnchorElement | null;
  if (!a || a.target || a.hasAttribute('download') || a.origin !== location.origin || a.pathname.startsWith('/api/')) return;
  e.preventDefault();
  navigate(a.pathname + a.search);
});

/** Premier segment de l'adresse : « catalogue », « admin »… */
export function section(): string {
  return route.path.split('/')[1] ?? '';
}
