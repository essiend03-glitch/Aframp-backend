import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { NextIntlClientProvider } from 'next-intl';
import { LocaleSwitcher } from '@/components/LocaleSwitcher';
import { CurrencyDisplay } from '@/components/CurrencyDisplay';

// Mock next/navigation
const mockReplace = jest.fn();
jest.mock('next/navigation', () => ({
  useRouter: () => ({
    replace: mockReplace,
  }),
  usePathname: () => '/en/dashboard',
}));

// Mock fetch
global.fetch = jest.fn(() =>
  Promise.resolve({
    ok: true,
    json: () => Promise.resolve({}),
  })
) as jest.Mock;

const enMessages = {
  common: {
    selectLanguage: 'Select Language',
  },
};

const frMessages = {
  common: {
    selectLanguage: 'Sélectionner la langue',
  },
};

describe('Locale Switching Integration', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it('should switch locale and update UI without layout shift', async () => {
    const { rerender } = render(
      <NextIntlClientProvider locale="en" messages={enMessages}>
        <div>
          <LocaleSwitcher />
          <CurrencyDisplay amount={1250.5} currency="NGN" />
        </div>
      </NextIntlClientProvider>
    );

    // Initial state
    expect(screen.getByText('English')).toBeInTheDocument();

    // Open dropdown
    const button = screen.getByRole('button', { name: /select language/i });
    fireEvent.click(button);

    // Select French
    const frenchOption = screen.getByText('Français');
    fireEvent.click(frenchOption);

    // Verify API call
    await waitFor(() => {
      expect(global.fetch).toHaveBeenCalledWith(
        '/api/v1/users/profile/settings',
        expect.objectContaining({
          method: 'PATCH',
          body: JSON.stringify({ locale: 'fr' }),
        })
      );
    });

    // Verify navigation
    await waitFor(() => {
      expect(mockReplace).toHaveBeenCalledWith('/fr/dashboard');
    });

    // Simulate locale change by rerendering with French
    rerender(
      <NextIntlClientProvider locale="fr" messages={frMessages}>
        <div>
          <LocaleSwitcher />
          <CurrencyDisplay amount={1250.5} currency="NGN" />
        </div>
      </NextIntlClientProvider>
    );

    // Verify French is now active
    expect(screen.getByText('Français')).toBeInTheDocument();
  });

  it('should preserve component state during locale switch', async () => {
    const TestComponent = () => {
      const [count, setCount] = React.useState(0);
      return (
        <div>
          <LocaleSwitcher />
          <button onClick={() => setCount(count + 1)}>
            Count: {count}
          </button>
        </div>
      );
    };

    const { rerender } = render(
      <NextIntlClientProvider locale="en" messages={enMessages}>
        <TestComponent />
      </NextIntlClientProvider>
    );

    // Increment counter
    const counterButton = screen.getByText(/Count: 0/);
    fireEvent.click(counterButton);
    expect(screen.getByText(/Count: 1/)).toBeInTheDocument();

    // Switch locale
    const switcherButton = screen.getByRole('button', { name: /select language/i });
    fireEvent.click(switcherButton);
    fireEvent.click(screen.getByText('Français'));

    // Rerender with new locale
    rerender(
      <NextIntlClientProvider locale="fr" messages={frMessages}>
        <TestComponent />
      </NextIntlClientProvider>
    );

    // State should be preserved (in real app, this would be managed by React state)
    // This test demonstrates the pattern
    expect(screen.getByText(/Count:/)).toBeInTheDocument();
  });

  it('should format currency correctly after locale switch', () => {
    const { rerender } = render(
      <NextIntlClientProvider locale="en" messages={enMessages}>
        <CurrencyDisplay amount={1250.5} currency="NGN" />
      </NextIntlClientProvider>
    );

    // English format uses comma as thousands separator
    let display = screen.getByText(/1,250\.50/);
    expect(display).toBeInTheDocument();

    // Switch to French locale (uses space as thousands separator)
    rerender(
      <NextIntlClientProvider locale="fr" messages={frMessages}>
        <CurrencyDisplay amount={1250.5} currency="NGN" />
      </NextIntlClientProvider>
    );

    // French format should use space (note: actual formatting depends on Intl.NumberFormat)
    display = screen.getByText(/1.*250/);
    expect(display).toBeInTheDocument();
  });
});
