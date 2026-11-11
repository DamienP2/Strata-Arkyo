<script lang="ts">
  import { api } from '../../lib/api';
  import { dateTime } from '../../lib/format';
  import type { AuditEntry, Member } from '../../lib/types';

  let entries = $state<AuditEntry[]>([]);
  let names = $state<Record<string, string>>({});
  let error = $state<string | null>(null);
  const ACTIONS: Record<string, string> = { create: 'Création', update: 'Modification', delete: 'Suppression', duplicate: 'Duplication', restore: 'Restauration', import: 'Import', exchange: 'Échange', login: 'Connexion', remove: 'Retrait', reset_password: 'Réinitialisation' };
  const OBJECTS: Record<string, string> = { account: 'Compte', role: 'Rôle', catalog_item: 'Fiche du catalogue', catalog: 'Catalogue', catalog_snapshot: 'Instantané du catalogue', site: 'Site' };
  const obj = (t: string) => OBJECTS[t] ?? (t.startsWith('ref:') ? `Référentiel (${t.slice(4)})` : t);
  const label = (e: AuditEntry) => { const a = (e.after ?? e.before) as any; return a?.name ?? a?.label ?? a?.file ?? ''; };

  Promise.all([api.get<{ entries: AuditEntry[] }>('/audit?limit=300'), api.get<{ members: Member[] }>('/members?removed=true')])
    .then(([a, m]) => { entries = a.entries; names = Object.fromEntries(m.members.map((x) => [x.id, x.name])); })
    .catch((e) => (error = e.message));
</script>

{#if error}<div class="alert err">{error}</div>{:else}
  <section class="card" style="padding:6px 18px">
    <div class="tbl-wrap">
      <table class="tbl">
        <thead><tr><th>Quand</th><th>Qui</th><th>Action</th><th>Objet</th></tr></thead>
        <tbody>
          {#each entries as e (e.id)}
            <tr><td class="mono" style="white-space:nowrap">{dateTime(e.at)}</td><td>{e.actor_id ? (names[e.actor_id] ?? 'Compte supprimé') : 'Système'}</td><td>{ACTIONS[e.action] ?? e.action}</td><td>{obj(e.object_type)}{#if label(e)} · <b>{label(e)}</b>{/if}</td></tr>
          {:else}<tr><td colspan="4" class="empty">Le journal est vide.</td></tr>{/each}
        </tbody>
      </table>
    </div>
  </section>
{/if}
