import { useTranslations } from 'next-intl';
import { LocaleSwitcher } from '@/components/LocaleSwitcher';

export default function HomePage() {
  const t = useTranslations('common');

  return (
    <main className="container">
      <header>
        <h1>Aframp Platform</h1>
        <LocaleSwitcher />
      </header>

      <section className="hero">
        <h2>{t('loading')}</h2>
        <p>Multi-currency, multi-language financial platform for Africa</p>
      </section>

      <style jsx>{`
        .container {
          max-width: 1200px;
          margin: 0 auto;
          padding: 24px;
        }

        header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 48px;
        }

        h1 {
          font-size: 24px;
          font-weight: 700;
          color: #1f2937;
        }

        .hero {
          text-align: center;
          padding: 64px 24px;
        }

        .hero h2 {
          font-size: 48px;
          font-weight: 800;
          color: #111827;
          margin-bottom: 16px;
        }

        .hero p {
          font-size: 20px;
          color: #6b7280;
        }
      `}</style>
    </main>
  );
}
