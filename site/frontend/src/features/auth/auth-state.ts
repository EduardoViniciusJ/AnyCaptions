import { currentAccount, type Account } from './auth-api';

export async function showAccount(element: HTMLElement) {
  const account: Account | null = await currentAccount();
  element.replaceChildren();
  const link = document.createElement('a');
  link.href = '/account';
  link.className = 'nav-account';
  if (account?.avatarUrl) {
    const image = document.createElement('img');
    image.src = account.avatarUrl;
    image.alt = '';
    image.referrerPolicy = 'no-referrer';
    link.append(image);
  }
  link.append(document.createTextNode(account?.name ?? 'Entrar'));
  element.append(link);
}
