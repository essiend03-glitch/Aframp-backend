'use client';

import { useLocale, useTranslations } from 'next-intl';
import { useRouter, usePathname } from 'next/navigation';
import { LOCALE_CONFIGS, SUPPORTED_LOCALES } from '@/config/locales';
import { SupportedLocale } from '@/types/locale';
import { useState, useTransition } from 'react';

export function LocaleSwitcher() {
  const t = useTranslations('common');
  const locale = useLocale() as SupportedLocale;
  const router = useRouter();
  const pathname = usePathname();
  const [isPending, startTransition] = useTransition();
  const [isOpen, setIsOpen] = useState(false);

  const handleLocaleChange = async (newLocale: SupportedLocale) => {
    if (newLocale === locale) {
      setIsOpen(false);
      return;
    }

    // Update user preference in backend
    try {
      await fetch('/api/v1/users/profile/settings', {
        method: 'PATCH',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          locale: newLocale,
        }),
      });
    } catch (error) {
      console.error('Failed to update locale preference:', error);
    }

    // Navigate to new locale path
    startTransition(() => {
      const newPath = pathname.replace(`/${locale}`, `/${newLocale}`);
      router.replace(newPath);
      setIsOpen(false);
    });
  };

  return (
    <div className="locale-switcher">
      <button
        onClick={() => setIsOpen(!isOpen)}
        disabled={isPending}
        className="locale-switcher-button"
        aria-label={t('selectLanguage')}
      >
        <span className="locale-flag">{LOCALE_CONFIGS[locale].nativeName}</span>
        <svg
          className={`locale-arrow ${isOpen ? 'open' : ''}`}
          width="12"
          height="12"
          viewBox="0 0 12 12"
          fill="none"
        >
          <path
            d="M2 4L6 8L10 4"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      </button>

      {isOpen && (
        <div className="locale-dropdown">
          {SUPPORTED_LOCALES.map((loc) => (
            <button
              key={loc}
              onClick={() => handleLocaleChange(loc)}
              className={`locale-option ${loc === locale ? 'active' : ''}`}
              disabled={isPending}
            >
              <span className="locale-name">{LOCALE_CONFIGS[loc].nativeName}</span>
              <span className="locale-code">{loc.toUpperCase()}</span>
            </button>
          ))}
        </div>
      )}

      <style jsx>{`
        .locale-switcher {
          position: relative;
          display: inline-block;
        }

        .locale-switcher-button {
          display: flex;
          align-items: center;
          gap: 8px;
          padding: 8px 12px;
          background: white;
          border: 1px solid #e5e7eb;
          border-radius: 8px;
          cursor: pointer;
          font-size: 14px;
          transition: all 0.2s;
        }

        .locale-switcher-button:hover {
          border-color: #d1d5db;
          background: #f9fafb;
        }

        .locale-switcher-button:disabled {
          opacity: 0.6;
          cursor: not-allowed;
        }

        .locale-arrow {
          transition: transform 0.2s;
        }

        .locale-arrow.open {
          transform: rotate(180deg);
        }

        .locale-dropdown {
          position: absolute;
          top: calc(100% + 4px);
          right: 0;
          min-width: 200px;
          background: white;
          border: 1px solid #e5e7eb;
          border-radius: 8px;
          box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1);
          z-index: 50;
          overflow: hidden;
        }

        .locale-option {
          display: flex;
          justify-content: space-between;
          align-items: center;
          width: 100%;
          padding: 12px 16px;
          background: white;
          border: none;
          cursor: pointer;
          font-size: 14px;
          transition: background 0.2s;
          text-align: left;
        }

        .locale-option:hover {
          background: #f9fafb;
        }

        .locale-option.active {
          background: #eff6ff;
          color: #2563eb;
        }

        .locale-option:disabled {
          opacity: 0.6;
          cursor: not-allowed;
        }

        .locale-code {
          font-size: 12px;
          color: #6b7280;
        }

        .locale-option.active .locale-code {
          color: #2563eb;
        }
      `}</style>
    </div>
  );
}
