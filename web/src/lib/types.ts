// Formes renvoyées par l'API du moteur.

export type Level = 'aucun' | 'lecture' | 'ecriture' | 'admin';
export type Domain = 'projets' | 'conception' | 'catalogue' | 'membres' | 'parametres';
export type Permissions = Record<Domain, Level>;

export interface Account {
  id: string; email: string; username: string | null; name: string; role_id: string;
  active: boolean; removed: boolean; must_change_password: boolean;
  phone: string | null; job: string | null; lang: string; theme: string;
  rev: number; created_at: string; updated_at: string;
}
export interface Role {
  id: string; name: string; color: string; builtin: boolean; locked: boolean;
  permissions: Permissions; rev: number; active_members: number;
}
export interface Me { account: Account; role: Role; is_owner: boolean; permissions: Permissions; must_change_password: boolean }
export interface Member extends Account { role_name: string }
export interface AuditEntry { id: number; at: string; actor_id: string | null; action: string; object_type: string; object_id: string; before: unknown; after: unknown }
