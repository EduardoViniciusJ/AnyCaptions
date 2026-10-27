import { useEffect, useState } from 'react';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Separator } from '@/components/ui/separator';
import { Skeleton } from '@/components/ui/skeleton';
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
    <Card className="account-panel">
      <CardHeader>
        <CardDescription>{t.kicker}</CardDescription>
        <CardTitle><h1>{account ? t.profile : t.welcome}</h1></CardTitle>
      </CardHeader>
      <CardContent>
        <div id="account-content" aria-live="polite" aria-busy={loading}>
          {loading ? <div role="status">
            <span className="sr-only">{t.checking}</span>
            <div className="grid gap-3" aria-hidden="true">
              <Skeleton className="h-12 w-full" />
              <Skeleton className="h-12 w-full" />
              <Skeleton className="h-12 w-full" />
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
                <Button asChild variant="outline" size="lg">
                  <a href={`${homePath}#download`}>{t.download}</a>
                </Button>
                <Button variant="outline" size="lg" disabled={signingOut} onClick={() => void signOut()}>
                  {signingOut ? t.signingOut : t.signOut}
                </Button>
              </div>
              <Separator className="my-6" />
              <div className="account-danger">
                {confirmingDelete ? (
                  <>
                    <p>{t.deleteWarning}</p>
                    <div className="account-actions">
                      <Button variant="destructive" size="lg" disabled={deleting} onClick={() => void removeAccount()}>
                        {deleting ? t.deleting : t.deleteConfirm}
                      </Button>
                      <Button variant="outline" size="lg" disabled={deleting} onClick={() => setConfirmingDelete(false)}>
                        {t.cancel}
                      </Button>
                    </div>
                  </>
                ) : (
                  <Button variant="ghost" size="sm" className="account-delete-link" onClick={() => setConfirmingDelete(true)}>
                    {t.deleteLink}
                  </Button>
                )}
              </div>
            </>
          ) : (
            <>
              <div className="login-options">
                {providers.map(provider => (
                  <Button key={provider.id} asChild variant="outline" className={`login-button login-${provider.id}`}>
                    <a href={loginUrl(provider.id)}>
                      <img className="provider-icon" src={provider.icon} alt="" width={20} height={20} />
                      <span>{t.signInWith.replace('{provider}', provider.label)}</span>
                    </a>
                  </Button>
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
        {error && <Alert variant="destructive" className="mt-6"><AlertDescription>{error}</AlertDescription></Alert>}
      </CardContent>
    </Card>
  );
}
