export interface Account {
  id: string;
  name: string;
  email: string | null;
  avatarUrl: string | null;
}

const baseUrl = (import.meta.env.PUBLIC_API_URL || '/api').replace(/\/$/, '');

export const loginUrl = (provider: 'google' | 'github' | 'discord') =>
  `${baseUrl}/auth/login/${provider}`;

export async function currentAccount(): Promise<Account | null> {
  try {
    const response = await fetch(`${baseUrl}/auth/me`, { credentials: 'include' });
    return response.ok ? await response.json() as Account : null;
  } catch {
    return null;
  }
}

export async function logout(): Promise<boolean> {
  const response = await fetch(`${baseUrl}/auth/logout`, {
    method: 'POST',
    credentials: 'include',
  });
  return response.ok;
}
