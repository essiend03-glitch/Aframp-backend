import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { LocaleSwitcher } from '@/components/LocaleSwitcher';
import { NextIntlClientProvider } from 'next-intl';

// Mock next/navigation
jest.mock('next/navigation', () => ({
  useRouter: () => ({
    replace: jest.fn(),
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

const messages = {
  common: {
    selectLanguage: 'Select Language',
  },
};

describe('LocaleSwitcher', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it('should render current locale', () => {
    render(
      <NextIntlClientProvider locale="en" messages={messages}>
        <LocaleSwitcher />
      </NextIntlClientProvider>
    );

    expect(screen.getByText('English')).toBeInTheDocument();
  });

  it('should open dropdown on click', () => {
    render(
      <NextIntlClientProvider locale="en" messages={messages}>
        <LocaleSwitcher />
      </NextIntlClientProvider>
    );

    const button = screen.getByRole('button', { name: /select language/i });
    fireEvent.click(button);

    expect(screen.getByText('Français')).toBeInTheDocument();
    expect(screen.getByText('Kiswahili')).toBeInTheDocument();
  });

  it('should call API when locale changes', async () => {
    render(
      <NextIntlClientProvider locale="en" messages={messages}>
        <LocaleSwitcher />
      </NextIntlClientProvider>
    );

    const button = screen.getByRole('button', { name: /select language/i });
    fireEvent.click(button);

    const frenchOption = screen.getByText('Français');
    fireEvent.click(frenchOption);

    await waitFor(() => {
      expect(global.fetch).toHaveBeenCalledWith(
        '/api/v1/users/profile/settings',
        expect.objectContaining({
          method: 'PATCH',
          body: JSON.stringify({ locale: 'fr' }),
        })
      );
    });
  });

  it('should not call API when selecting current locale', async () => {
    render(
      <NextIntlClientProvider locale="en" messages={messages}>
        <LocaleSwitcher />
      </NextIntlClientProvider>
    );

    const button = screen.getByRole('button', { name: /select language/i });
    fireEvent.click(button);

    const englishOption = screen.getByText('English');
    fireEvent.click(englishOption);

    await waitFor(() => {
      expect(global.fetch).not.toHaveBeenCalled();
    });
  });
});
