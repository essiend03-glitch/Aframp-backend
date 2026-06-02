'use client';

import { useState } from 'react';
import { InstitutionalUser, InstitutionalRole } from '@/types';
import { RbacGate } from '../layout/RbacGate';

interface ConfigManagerPanelProps {
  users: InstitutionalUser[];
  onRoleChange: (userId: string, role: InstitutionalRole) => Promise<void>;
  onToggleActive: (userId: string, active: boolean) => Promise<void>;
  onUpdateIpWhitelist: (userId: string, ips: string[]) => Promise<void>;
}

const ROLES: InstitutionalRole[] = ['SuperAdmin', 'Operator', 'ComplianceAuditor', 'Signatory'];

export function ConfigManagerPanel({
  users,
  onRoleChange,
  onToggleActive,
  onUpdateIpWhitelist,
}: ConfigManagerPanelProps) {
  const [editingIps, setEditingIps] = useState<Record<string, string>>({});

  function handleIpEdit(userId: string, current: string[]) {
    setEditingIps(prev => ({ ...prev, [userId]: current.join(', ') }));
  }

  async function handleIpSave(userId: string) {
    const raw = editingIps[userId] ?? '';
    const ips = raw.split(',').map(s => s.trim()).filter(Boolean);
    await onUpdateIpWhitelist(userId, ips);
    setEditingIps(prev => { const n = { ...prev }; delete n[userId]; return n; });
  }

  return (
    <section className="config-panel" aria-label="User configuration">
      <header className="config-panel__header">
        <h2>User &amp; Access Configuration</h2>
        <p className="config-panel__subtitle">
          Manage team roles, permissions, and IP whitelisting.
        </p>
      </header>

      <div className="config-panel__table-wrap">
        <table className="data-table" role="table">
          <thead>
            <tr>
              <th scope="col">Name</th>
              <th scope="col">Email</th>
              <th scope="col">Role</th>
              <th scope="col">Signer Weight</th>
              <th scope="col">IP Whitelist</th>
              <th scope="col">Status</th>
              <th scope="col"><span className="sr-only">Actions</span></th>
            </tr>
          </thead>
          <tbody>
            {users.map(user => (
              <tr key={user.id} data-testid={`user-row-${user.id}`}>
                <td className="font-mono-sm">{user.name}</td>
                <td>{user.email}</td>

                <td>
                  <RbacGate
                    permission="users:write"
                    fallback={<span className="badge badge--role">{user.role}</span>}
                  >
                    <select
                      aria-label={`Role for ${user.name}`}
                      value={user.role}
                      onChange={e => onRoleChange(user.id, e.target.value as InstitutionalRole)}
                      className="select-inline"
                    >
                      {ROLES.map(r => <option key={r} value={r}>{r}</option>)}
                    </select>
                  </RbacGate>
                </td>

                <td className="text-center">{user.signerWeight}</td>

                <td>
                  {editingIps[user.id] !== undefined ? (
                    <div className="ip-edit-row">
                      <input
                        aria-label={`IP whitelist for ${user.name}`}
                        value={editingIps[user.id]}
                        onChange={e => setEditingIps(prev => ({ ...prev, [user.id]: e.target.value }))}
                        placeholder="192.168.1.1, 10.0.0.0/8"
                        className="input-inline"
                      />
                      <button className="btn btn--xs btn--primary" onClick={() => handleIpSave(user.id)}>Save</button>
                      <button className="btn btn--xs btn--ghost" onClick={() => setEditingIps(prev => { const n = { ...prev }; delete n[user.id]; return n; })}>Cancel</button>
                    </div>
                  ) : (
                    <RbacGate permission="config:write" fallback={<span className="text-muted">{user.ipWhitelist.join(', ') || 'Any'}</span>}>
                      <button className="btn btn--xs btn--ghost" onClick={() => handleIpEdit(user.id, user.ipWhitelist)}>
                        {user.ipWhitelist.length > 0 ? user.ipWhitelist.join(', ') : 'Any — edit'}
                      </button>
                    </RbacGate>
                  )}
                </td>

                <td>
                  <RbacGate permission="users:write" fallback={
                    <span className={`badge ${user.isActive ? 'badge--success' : 'badge--muted'}`}>
                      {user.isActive ? 'Active' : 'Suspended'}
                    </span>
                  }>
                    <button
                      className={`toggle-btn ${user.isActive ? 'toggle-btn--on' : 'toggle-btn--off'}`}
                      onClick={() => onToggleActive(user.id, !user.isActive)}
                      aria-label={user.isActive ? `Suspend ${user.name}` : `Activate ${user.name}`}
                    >
                      {user.isActive ? 'Active' : 'Suspended'}
                    </button>
                  </RbacGate>
                </td>

                <td />
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}
