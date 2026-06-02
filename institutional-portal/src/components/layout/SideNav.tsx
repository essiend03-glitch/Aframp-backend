'use client';

import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { useRbac } from './RbacGate';
import { hasPermission, InstitutionalRole } from '@/types';

interface NavItem {
  href: string;
  label: string;
  icon: string;
  permission: string;
}

const NAV_ITEMS: NavItem[] = [
  { href: '/dashboard',       label: 'Overview',         icon: '▤', permission: 'proposals:read' },
  { href: '/pending-actions', label: 'Pending Actions',  icon: '⏳', permission: 'proposals:read' },
  { href: '/compliance',      label: 'Compliance Trail', icon: '📋', permission: 'compliance:read' },
  { href: '/config',          label: 'Configuration',    icon: '⚙',  permission: 'config:read' },
];

export function SideNav() {
  const pathname = usePathname();
  const { role, userName } = useRbac();

  const visible = NAV_ITEMS.filter(item => hasPermission(role as InstitutionalRole, item.permission));

  return (
    <aside className="side-nav">
      <div className="side-nav__brand">
        <span className="side-nav__logo">◈ Aframp</span>
        <span className="side-nav__portal-label">Institutional</span>
      </div>

      <nav className="side-nav__links" aria-label="Main navigation">
        {visible.map(item => (
          <Link
            key={item.href}
            href={item.href}
            className={`side-nav__link${pathname?.startsWith(item.href) ? ' side-nav__link--active' : ''}`}
            aria-current={pathname?.startsWith(item.href) ? 'page' : undefined}
          >
            <span aria-hidden="true">{item.icon}</span>
            {item.label}
          </Link>
        ))}
      </nav>

      <div className="side-nav__footer">
        <span className="side-nav__user">{userName}</span>
        <span className="side-nav__role-badge" data-role={role}>{role}</span>
      </div>
    </aside>
  );
}
