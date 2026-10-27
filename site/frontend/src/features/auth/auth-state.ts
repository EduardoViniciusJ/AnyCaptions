import { currentAccount, type Account } from './auth-api';

export async function showAccount(element: HTMLElement) {
  const account: Account | null = await currentAccount();
  const href = element.dataset.href ?? '/account';
  const label = element.dataset.label ?? 'Entrar';
  element.replaceChildren();
  const link = document.createElement('a');
  link.href = href;
  link.className = 'nav-account nav-pill';
  if (account?.avatarUrl) {
    const image = document.createElement('img');
    image.src = account.avatarUrl;
    image.alt = '';
    image.referrerPolicy = 'no-referrer';
    link.append(image);
  }
  link.append(document.createTextNode(account?.name ?? label));
  element.append(link);
}
