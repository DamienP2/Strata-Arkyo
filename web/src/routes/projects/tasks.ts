// Outils partagés par les vues des tâches (Kanban, Timeline, Liste).
import { TASK_STATUS, PRIORITY, type Task, type TaskStatus } from '../../lib/projects';

/** Statuts dans l'ordre des colonnes du Kanban. */
export const STATUS_ORDER = Object.keys(TASK_STATUS) as TaskStatus[];

/** Repère de couleur d'un statut (point de colonne, barre de Timeline). « En cours » suit le projet. */
export const STATUS_COLOR: Record<TaskStatus, string> = {
  todo: 'var(--info)', in_progress: 'var(--st-cours)', done: 'var(--ok)', validated: 'var(--st-pret)',
};

export const isOpen = (t: Task) => t.status === 'todo' || t.status === 'in_progress';
/** Échéance dépassée sur une tâche encore à traiter. */
export const isLate = (t: Task, today: string) => !!t.due && t.due < today && isOpen(t);

const PRIO_RANK = Object.keys(PRIORITY);
/** Ordre d'une colonne : priorité décroissante, puis échéance la plus proche (sans échéance à la fin). */
export function byPriorityThenDue(a: Task, b: Task): number {
  const p = PRIO_RANK.indexOf(b.priority) - PRIO_RANK.indexOf(a.priority);
  if (p) return p;
  if (a.due !== b.due) return !a.due ? 1 : !b.due ? -1 : a.due.localeCompare(b.due);
  return a.position - b.position;
}

/** Corps complet attendu par PATCH /tasks/{id} (l'API remplace toutes les données de la tâche). */
export function taskBody(t: Task) {
  const { title, description, start_date, due, done_on, assignee_id, status, priority } = t;
  return { title, description, start_date: start_date ?? null, due, done_on, assignee_id, status, priority };
}

/** Jour AAAA-MM-JJ décalé de n jours (calcul à midi UTC, sans effet d'heure d'été). */
export function addDays(d: string, n: number): string {
  const t = new Date(d + 'T12:00:00Z');
  t.setUTCDate(t.getUTCDate() + n);
  return t.toISOString().slice(0, 10);
}
/** Jour de la semaine, lundi = 0. */
export const weekday = (d: string) => (new Date(d + 'T12:00:00Z').getUTCDay() + 6) % 7;
/** Numéro de semaine ISO. */
export function isoWeek(d: string): number {
  const t = new Date(d + 'T12:00:00Z');
  t.setUTCDate(t.getUTCDate() + 3 - weekday(d));
  const jan4 = new Date(Date.UTC(t.getUTCFullYear(), 0, 4, 12));
  return 1 + Math.round(((t.getTime() - jan4.getTime()) / 86_400_000 - 3 + ((jan4.getUTCDay() + 6) % 7)) / 7);
}
