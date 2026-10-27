import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { AccountPanel } from './AccountPanel';
import { currentAccount, deleteAccount, logout } from './auth-api';

vi.mock('./auth-api', () => ({
  currentAccount: vi.fn(),
  logout: vi.fn(),
  deleteAccount: vi.fn(),
  loginUrl: (provider: string) => `/api/auth/login/${provider}`,
}));

beforeEach(() => {
  vi.clearAllMocks();
  history.replaceState({}, '', '/account');
  vi.mocked(currentAccount).mockResolvedValue(null);
});

afterEach(cleanup);

describe('AccountPanel', () => {
  it('aguarda a sessão e oferece os três provedores com os endpoints existentes', async () => {
    render(<AccountPanel />);
    expect(screen.getByRole('status').textContent).toBe('Verificando sessão…');

    for (const [provider, label] of [['discord', 'Discord'], ['google', 'Google'], ['github', 'GitHub']]) {
      const link = await screen.findByRole('link', { name: `Entrar com ${label}` });
      expect(link.getAttribute('href')).toBe(`/api/auth/login/${provider}`);
      expect(link.querySelector('img')?.getAttribute('src')).toMatch(/^\/icons\//);
    }
    expect(screen.queryByRole('status')).toBeNull();
  });

  it('mostra dados da conta como texto e permite tentar sair novamente após uma falha', async () => {
    const name = '<img src=x onerror=alert(1)>';
    vi.mocked(currentAccount).mockResolvedValue({ id: '1', name, email: null, avatarUrl: null });
    vi.mocked(logout).mockResolvedValue(false);
    render(<AccountPanel />);

    expect((await screen.findByText(name)).tagName).toBe('STRONG');
    expect(document.querySelector('img[onerror]')).toBeNull();
    expect(screen.queryByRole('link', { name: 'Entrar com Discord' })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Sair da conta' }));
    expect((await screen.findByRole('alert')).textContent).toContain('Não foi possível sair. Tente novamente.');
    expect((screen.getByRole('button', { name: 'Sair da conta' }) as HTMLButtonElement).disabled).toBe(false);
    expect(logout).toHaveBeenCalledOnce();
  });

  it('apresenta a falha de login sem ocultar as opções para tentar novamente', async () => {
    history.replaceState({}, '', '/account?login=failed');
    render(<AccountPanel />);
    expect((await screen.findByRole('alert')).textContent).toContain('Não foi possível entrar. Tente novamente.');
    expect(await screen.findByRole('link', { name: 'Entrar com Google' })).toBeTruthy();
  });

  it('exibe o aviso de consentimento com o link da política junto das opções de login', async () => {
    render(<AccountPanel />);
    await screen.findByRole('link', { name: 'Entrar com Google' });
    const consent = screen.getByText(/você concorda/i);
    expect(consent.querySelector('a')?.getAttribute('href')).toBe('/docs#privacidade');
  });

  it('só exclui a conta depois da confirmação e mantém a conta ao cancelar', async () => {
    vi.mocked(currentAccount).mockResolvedValue({ id: '1', name: 'Ana', email: 'ana@exemplo.com', avatarUrl: null });
    vi.mocked(deleteAccount).mockResolvedValue(false);
    render(<AccountPanel />);

    fireEvent.click(await screen.findByRole('button', { name: 'Excluir minha conta' }));
    expect(deleteAccount).not.toHaveBeenCalled();
    expect(screen.getByText(/apagados de forma permanente/)).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: 'Cancelar' }));
    expect(screen.queryByText(/apagados de forma permanente/)).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'Excluir minha conta' }));
    fireEvent.click(screen.getByRole('button', { name: 'Excluir de forma permanente' }));
    expect((await screen.findByRole('alert')).textContent).toContain('Não foi possível excluir a conta.');
    expect(deleteAccount).toHaveBeenCalledOnce();
  });
});
