import { mount } from 'svelte';
// Polices embarquées : le site fonctionne sans accès à Internet.
import '@fontsource-variable/bricolage-grotesque/opsz.css';
import '@fontsource-variable/inter/index.css';
import '@fontsource/jetbrains-mono/latin-500.css';
import '@fontsource/jetbrains-mono/latin-600.css';
import '@fontsource/jetbrains-mono/latin-700.css';
import './styles/strates.css';
import './styles/app.css';
import './styles/refine.css';
import './styles/harmonie.css';
import App from './App.svelte';
import { applyTheme } from './lib/theme.svelte';

applyTheme();
mount(App, { target: document.getElementById('app')! });
