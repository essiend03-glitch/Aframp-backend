'use client';

import { useTranslations, useLocale } from 'next-intl';
import { CurrencyDisplay } from './CurrencyDisplay';
import { dateFormatter } from '@/lib/formatters/date-formatter';
import { CurrencyCode, SupportedLocale } from '@/types/locale';

interface Transaction {
  id: string;
  amount: number;
  currency: CurrencyCode;
  status: 'pending' | 'completed' | 'failed' | 'cancelled';
  type: 'onramp' | 'offramp' | 'transfer';
  timestamp: string;
  reference: string;
}

interface TransactionListProps {
  transactions: Transaction[];
}

export function TransactionList({ transactions }: TransactionListProps) {
  const t = useTranslations('transactions');
  const locale = useLocale() as SupportedLocale;

  if (transactions.length === 0) {
    return (
      <div className="empty-state">
        <p>{t('noTransactions')}</p>
      </div>
    );
  }

  return (
    <div className="transaction-list">
      <table>
        <thead>
          <tr>
            <th>{t('date')}</th>
            <th>{t('type')}</th>
            <th>{t('amount')}</th>
            <th>{t('status')}</th>
            <th>{t('reference')}</th>
          </tr>
        </thead>
        <tbody>
          {transactions.map((tx) => (
            <tr key={tx.id}>
              <td>
                {dateFormatter.formatDateTime(tx.timestamp, locale)}
              </td>
              <td>{t(tx.type)}</td>
              <td>
                <CurrencyDisplay amount={tx.amount} currency={tx.currency} />
              </td>
              <td>
                <span className={`status status-${tx.status}`}>
                  {t(tx.status)}
                </span>
              </td>
              <td className="reference">{tx.reference}</td>
            </tr>
          ))}
        </tbody>
      </table>

      <style jsx>{`
        .transaction-list {
          width: 100%;
          overflow-x: auto;
        }

        table {
          width: 100%;
          border-collapse: collapse;
          background: white;
          border-radius: 8px;
          overflow: hidden;
        }

        thead {
          background: #f9fafb;
        }

        th {
          padding: 12px 16px;
          text-align: left;
          font-weight: 600;
          font-size: 14px;
          color: #374151;
          border-bottom: 1px solid #e5e7eb;
        }

        td {
          padding: 12px 16px;
          font-size: 14px;
          color: #1f2937;
          border-bottom: 1px solid #f3f4f6;
        }

        tbody tr:hover {
          background: #f9fafb;
        }

        .status {
          display: inline-block;
          padding: 4px 8px;
          border-radius: 4px;
          font-size: 12px;
          font-weight: 500;
        }

        .status-pending {
          background: #fef3c7;
          color: #92400e;
        }

        .status-completed {
          background: #d1fae5;
          color: #065f46;
        }

        .status-failed {
          background: #fee2e2;
          color: #991b1b;
        }

        .status-cancelled {
          background: #f3f4f6;
          color: #4b5563;
        }

        .reference {
          font-family: monospace;
          font-size: 12px;
          color: #6b7280;
        }

        .empty-state {
          padding: 48px 24px;
          text-align: center;
          color: #6b7280;
        }
      `}</style>
    </div>
  );
}
