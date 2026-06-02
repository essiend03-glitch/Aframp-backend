'use client';

import { SideNav } from './SideNav';
import { RbacProvider } from './RbacGate';
import { InstitutionalRole } from '@/types';

interface InstitutionalLayoutProps {
  role: InstitutionalRole;
  userId: string;
  userName: string;
  children: React.ReactNode;
}

export function InstitutionalLayout({ role, userId, userName, children }: InstitutionalLayoutProps) {
  return (
    <RbacProvider role={role} userId={userId} userName={userName}>
      <div className="institutional-layout">
        <SideNav />
        <main className="institutional-layout__main" id="main-content">
          {children}
        </main>
      </div>
    </RbacProvider>
  );
}
