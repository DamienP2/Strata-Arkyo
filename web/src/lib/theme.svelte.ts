// Thème sombre (par défaut), clair, ou celui du système, mémorisé sur cet appareil.
// Le sombre est le défaut du CSS ; le clair pose data-theme="light" sur <html>. Le script
// d'index.html applique le choix avant la première image.

export type Theme = 'dark' | 'light' | 'system';
export const THEMES: Theme[] = ['dark', 'light', 'system'];
const KEY = 'strata.theme';

function read(): Theme {
  try {
    const v = localStorage.getItem(KEY) ?? localStorage.getItem('sa-theme');
    return v === 'light' || v === 'system' || v === 'dark' ? v : 'dark';
  } catch { return 'dark'; }
}

export const theme = $state({ value: read() as Theme });

const mq = typeof matchMedia === 'undefined' ? null : matchMedia('(prefers-color-scheme: light)');

export function isDark(): boolean {
  return theme.value === 'dark' || (theme.value === 'system' && !mq?.matches);
}

export function applyTheme() {
  document.documentElement.dataset.theme = isDark() ? 'dark' : 'light';
}

mq?.addEventListener('change', () => { if (theme.value === 'system') applyTheme(); });

export function setTheme(t: Theme) {
  if (!THEMES.includes(t)) return;
  theme.value = t;
  try { localStorage.setItem(KEY, t); } catch { /* stockage indisponible */ }
  applyTheme();
}

export function toggleTheme() {
  setTheme(isDark() ? 'light' : 'dark');
}
