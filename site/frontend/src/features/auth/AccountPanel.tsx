import { useEffect, useState } from 'react';
import { localePath, messages, type Lang } from '@/i18n';
import { currentAccount, deleteAccount, loginUrl, logout, type Account } from './auth-api';

const providers = [
  { id: 'discord', label: 'Discord', icon: '/icons/discord.svg' },
  { id: 'google', label: 'Google', icon: '/icons/google.png' },
  { id: 'github', label: 'GitHub', icon: '/icons/github.svg' },
] as const;

export function AccountPanel({ lang = 'pt-br' }: { lang?: Lang }) {
  const t = messages[lang].account;
  const homePath = localePath(lang, '/');
  const [account, setAccount] = useState<Account | null>(null);
  const [loading, setLoading] = useState(true);
  const [signingOut, setSigningOut] = useState(false);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [error, setError] = useState('');

  useEffect(() => {
    let active = true;
    if (new URLSearchParams(location.search).get('login') === 'failed') {
      setError(t.errSignIn);
    }
    void currentAccount().then(user => {
      if (!active) return;
      setAccount(user);
      setLoading(false);
    });
    return () => { active = false; };
  }, [t.errSignIn]);

  async function signOut() {
    setSigningOut(true);
    setError('');
    try {
      if (!await logout()) throw new Error('Logout failed');
      location.href = homePath;
    } catch {
      setError(t.errSignOut);
      setSigningOut(false);
    }
  }

  async function removeAccount() {
    setDeleting(true);
    setError('');
    try {
      if (!await deleteAccount()) throw new Error('Delete failed');
      location.href = homePath;
    } catch {
      setError(t.errDelete);
      setDeleting(false);
      setConfirmingDelete(false);
    }
  }

  return (
    <div className="account-panel card card-border bg-base-200">
      <div className="card-body">
        <p className="kicker">{t.kicker}</p>
        <h1>{account ? t.profile : t.welcome}</h1>
        <div id="account-content" aria-live="polite" aria-busy={loading}>
          {loading ? <div role="status">
            <span className="sr-only">{t.checking}</span>
            <div className="grid gap-3" aria-hidden="true">
              <div className="skeleton h-12 w-full" />
              <div className="skeleton h-12 w-full" />
              <div className="skeleton h-12 w-full" />
              <div className="skeleton mt-1 h-3 w-full" />
              <div className="skeleton h-3 w-2/3" />
            </div>
          </div> : account ? (
            <>
              <div className="account-identity">
                {account.avatarUrl && <img src={account.avatarUrl} alt="" referrerPolicy="no-referrer" width={48} height={48} />}
                <div>
                  <strong>{account.name}</strong>
                  <span>{account.email ?? t.noEmail}</span>
                </div>
              </div>
              <p className="account-news">{t.news}</p>
              <div className="account-actions">
                <a className="btn hero-alt" href={`${homePath}#download`}>{t.download}</a>
                <button type="button" className="btn hero-alt" disabled={signingOut} onClick={() => void signOut()}>
                  {signingOut ? t.signingOut : t.signOut}
                </button>
              </div>
              <div className="divider my-6" />
              <div className="account-danger">
                {confirmingDelete ? (
                  <>
                    <p>{t.deleteWarning}</p>
                    <div className="account-actions">
                      <button type="button" className="btn hero-alt hero-danger" disabled={deleting} onClick={() => void removeAccount()}>
                        {deleting ? t.deleting : t.deleteConfirm}
                      </button>
                      <button type="button" className="btn hero-alt" disabled={deleting} onClick={() => setConfirmingDelete(false)}>
                        {t.cancel}
                      </button>
                    </div>
                  </>
                ) : (
                  <button type="button" className="btn btn-ghost btn-sm account-delete-link" onClick={() => setConfirmingDelete(true)}>
                    {t.deleteLink}
                  </button>
                )}
              </div>
            </>
          ) : (
            <>
              <div className="login-options">
                {providers.map(provider => (
                  <a key={provider.id} className={`btn btn-outline login-button login-${provider.id}`} href={loginUrl(provider.id)}>
                    <img className="provider-icon" src={provider.icon} alt="" width={20} height={20} />
                    <span>{t.signInWith.replace('{provider}', provider.label)}</span>
                  </a>
                ))}
              </div>
              <p className="login-consent">
                {t.consent.before}
                <a className="text-link" href={`${localePath(lang, '/docs')}#privacidade`}>{t.consent.link}</a>
                {t.consent.after}
              </p>
            </>
          )}
        </div>
        {error && <div role="alert" className="alert alert-error alert-soft mt-6"><span>{error}</span></div>}
      </div>
    </div>
  );
}
