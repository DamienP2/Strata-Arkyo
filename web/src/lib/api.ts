// Client de l'API du moteur. Toute écriture porte l'en-tête X-Strata-Request (protection CSRF).

export interface FieldError { field: string; message: string }

export class ApiError extends Error {
  status: number;
  code: string;
  fields: FieldError[];
  constructor(status: number, code: string, message: string, fields: FieldError[] = []) {
    super(message);
    this.status = status;
    this.code = code;
    this.fields = fields;
  }
  /** Message du champ nommé, s'il y en a un. */
  field(name: string): string | undefined {
    return this.fields.find((f) => f.field === name || f.field.startsWith(name + '.') || f.field.startsWith(name + '['))?.message;
  }
}

/** Identifiant de cet onglet, propre à la page chargée. */
export const CLIENT_ID = Math.random().toString(36).slice(2) + Date.now().toString(36);

type Listener = (e: ApiError) => void;
const unauthenticated: Listener[] = [];
/** Appelé quand la session a expiré : l'application revient à l'écran de connexion. */
export function onUnauthenticated(f: Listener) { unauthenticated.push(f); }

async function request<T>(method: string, path: string, body?: unknown, raw?: BodyInit, rawType?: string): Promise<T> {
  const headers: Record<string, string> = {};
  if (method !== 'GET') {
    headers['X-Strata-Request'] = '1';
    // Les événements temps réel reprennent cet identifiant : l'onglet ignore ses propres écritures.
    headers['X-Strata-Client'] = CLIENT_ID;
  }
  let payload: BodyInit | undefined;
  if (raw !== undefined) {
    payload = raw;
    headers['Content-Type'] = rawType ?? 'application/octet-stream';
  } else if (body !== undefined) {
    payload = JSON.stringify(body);
    headers['Content-Type'] = 'application/json';
  }
  let res: Response;
  try {
    res = await fetch('/api' + path, { method, headers, body: payload, credentials: 'same-origin' });
  } catch {
    throw new ApiError(0, 'network', 'Le serveur ne répond pas. Vérifiez la connexion au réseau du site.');
  }
  if (res.status === 204) return undefined as T;
  const text = await res.text();
  let data: any = null;
  try { data = text ? JSON.parse(text) : null; } catch { data = null; }
  if (!res.ok) {
    const e = new ApiError(res.status, data?.code ?? 'error', data?.message ?? `Erreur ${res.status}.`, data?.fields ?? []);
    if (res.status === 401 && path !== '/auth/login') unauthenticated.forEach((f) => f(e));
    throw e;
  }
  return data as T;
}

export const api = {
  get: <T>(p: string) => request<T>('GET', p),
  post: <T>(p: string, b?: unknown) => request<T>('POST', p, b ?? {}),
  put: <T>(p: string, b?: unknown) => request<T>('PUT', p, b),
  patch: <T>(p: string, b?: unknown) => request<T>('PATCH', p, b),
  del: <T>(p: string) => request<T>('DELETE', p),
  upload: <T>(p: string, data: Blob, type?: string) => request<T>('POST', p, undefined, data, type),
};

/** Lien de téléchargement direct (GET) : le navigateur gère le fichier. */
export function download(path: string) {
  const a = document.createElement('a');
  a.href = '/api' + path;
  a.rel = 'noopener';
  document.body.appendChild(a);
  a.click();
  a.remove();
}
