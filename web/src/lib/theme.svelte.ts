// Thème clair, sombre ou réglage du système, mémorisé sur cet appareil.

type Theme = 'light' | 'dark' | 'system';
function read(): Theme {
  try { return (localStorage.getItem('sa-theme') as Theme) || 'system'; } catch { return 'system'; }
}

export const theme = $state({ value: read() as Theme });

export function isDark(): boolean {
  return theme.value === 'dark' || (theme.value === 'system' && matchMedia('(prefers-color-scheme: dark)').matches);
}

export function applyTheme() {
  const el = document.documentElement;
  if (theme.value === 'system') delete el.dataset.theme;
  else el.dataset.theme = theme.value;
}

export function toggleTheme() {
  theme.value = isDark() ? 'light' : 'dark';
  try { localStorage.setItem('sa-theme', theme.value); } catch { /* stockage indisponible */ }
  applyTheme();
}
