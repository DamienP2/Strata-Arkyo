<script lang="ts">
  import { untrack } from 'svelte';
  import { api } from '../../lib/api';
  import { fail } from '../../lib/toast.svelte';
  import { dateTime } from '../../lib/format';
  import type { AuditEntry, Member } from '../../lib/types';

  let { projectId }: { projectId: string } = $props();
  let entries = $state<AuditEntry[]>([]);
  let names = $state<Record<string, string>>({});
  Promise.all([api.get<{ entries: AuditEntry[] }>(`/projects/${untrack(() => projectId)}/history?limit=300`), api.get<{ members: Member[] }>('/members?removed=true')])
    .then(([h, m]) => { entries = h.entries; names = Object.fromEntries(m.members.map((x) => [x.id, x.name])); })
    .catch(fail);

  const OBJ: Record<string, [string, string]> = {
    project: ['du projet', 'le projet'], location: ['d’un emplacement', 'l’emplacement'], container: ['d’un contenant', 'le contenant'],
    article: ['d’un article', 'l’article'], instance: ['d’un exemplaire', 'l’exemplaire'], task: ['d’une tâche', 'la tâche'],
  };
  const ACT: Record<string, string> = { create: 'Création', update: 'Modification', delete: 'Suppression', move: 'Déplacement' };
  function label(e: AuditEntry): string {
    const o = OBJ[e.object_type]?.[0] ?? e.object_type;
    return `${ACT[e.action] ?? e.action} ${o}`;
  }
  function what(e: AuditEntry): string {
    const a = (e.after ?? e.before) as any;
    if (!a) return '';
    if (e.object_type === 'instance') return `${a.tag}${a.serial ? ` · série ${a.serial}` : ''}${a.mac ? ` · ${a.mac}` : ''}`;
    if (e.object_type === 'article') {
      const b = e.before as any;
      return b && a && b.quantity !== a.quantity ? `quantité ${b.quantity} → ${a.quantity}` : `${a.quantity} ×`;
    }
    if (e.object_type === 'project' && e.before && e.after && (e.before as any).status !== a.status) return `statut ${(e.before as any).status} → ${a.status}`;
    return a.name ?? a.title ?? '';
  }
</script>

<section class="card" style="padding:6px 18px 10px">
  <div class="tbl-wrap"><table class="tbl">
    <thead><tr><th>Quand</th><th>Qui</th><th>Action</th><th>Détail</th></tr></thead>
    <tbody>
      {#each entries as e (e.id)}<tr><td class="mono small" style="white-space:nowrap">{dateTime(e.at)}</td><td>{e.actor_id ? (names[e.actor_id] ?? '—') : 'Système'}</td><td>{label(e)}</td><td class="small">{what(e)}</td></tr>
      {:else}<tr><td colspan="4" class="empty">Aucune modification enregistrée.</td></tr>{/each}
    </tbody>
  </table></div>
</section>
