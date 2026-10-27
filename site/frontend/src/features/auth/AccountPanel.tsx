import { useEffect, useState } from 'react';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Separator } from '@/components/ui/separator';
import { currentAccount, deleteAccount, loginUrl, logout, type Account } from './auth-api';

const providers = [
  { id: 'discord', label: 'Discord', icon: '/icons/discord.svg' },
  { id: 'google', label: 'Google', icon: '/icons/google.png' },
  { id: 'github', label: 'GitHub', icon: '/icons/github.svg' },
] as const;

export function AccountPanel() {
  const [account, setAccount] = useState<Account | null>(null);
  const [loading, setLoading] = useState(true);
  const [signingOut, setSigningOut] = useState(false);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [error, setError] = useState('');

  useEffect(() => {
    let active = true;
    if (new URLSearchParams(location.search).get('login') === 'failed') {
      setError('Não foi possível entrar. Tente novamente.');
    }
    void currentAccount().then(user => {
      if (!active) return;
      setAccount(user);
      setLoading(false);
    });
    return () => { active = false; };
  }, []);

  async function signOut() {
    setSigningOut(true);
    setError('');
    try {
      if (!await logout()) throw new Error('Logout failed');
      location.href = '/';
    } catch {
      setError('Não foi possível sair. Tente novamente.');
      setSigningOut(false);
    }
  }

  async function removeAccount() {
    setDeleting(true);
    setError('');
    try {
      if (!await deleteAccount()) throw new Error('Delete failed');
      location.href = '/';
    } catch {
      setError('Não foi possível excluir a conta. Tente novamente.');
      setDeleting(false);
      setConfirmingDelete(false);
    }
  }

  return (
    <Card className="account-panel">
      <CardHeader>
        <CardDescription>Sua conta</CardDescription>
        <CardTitle><h1>{account ? 'Seu perfil' : 'Bem-vindo ao AnyCaptions.'}</h1></CardTitle>
      </CardHeader>
      <CardContent>
        <div id="account-content" aria-live="polite" aria-busy={loading}>
          {loading ? <p role="status">Verificando sessão…</p> : account ? (
            <>
              <div className="account-identity">
                {account.avatarUrl && <img src={account.avatarUrl} alt="" referrerPolicy="no-referrer" width={48} height={48} />}
                <div>
                  <strong>{account.name}</strong>
                  <span>{account.email ?? 'E-mail não fornecido pelo provedor'}</span>
                </div>
              </div>
              <p className="account-news">Em breve: novidades do AnyCaptions aparecerão aqui no site.</p>
              <div className="account-actions">
                <Button asChild variant="outline" size="lg">
                  <a href="/#download">Baixar o aplicativo</a>
                </Button>
                <Button variant="outline" size="lg" disabled={signingOut} onClick={() => void signOut()}>
                  {signingOut ? 'Saindo…' : 'Sair da conta'}
                </Button>
              </div>
              <Separator className="my-6" />
              <div className="account-danger">
                {confirmingDelete ? (
                  <>
                    <p>A conta e o seu histórico de downloads serão apagados de forma permanente. O aplicativo já instalado continua funcionando.</p>
                    <div className="account-actions">
                      <Button variant="destructive" size="lg" disabled={deleting} onClick={() => void removeAccount()}>
                        {deleting ? 'Excluindo…' : 'Excluir de forma permanente'}
                      </Button>
                      <Button variant="outline" size="lg" disabled={deleting} onClick={() => setConfirmingDelete(false)}>
                        Cancelar
                      </Button>
                    </div>
                  </>
                ) : (
                  <Button variant="ghost" size="sm" className="account-delete-link" onClick={() => setConfirmingDelete(true)}>
                    Excluir minha conta
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
                      <span>Entrar com {provider.label}</span>
                    </a>
                  </Button>
                ))}
              </div>
              <p className="login-consent">
                Ao entrar, você concorda que o site guarde o nome, o e-mail e a foto da sua
                conta do provedor, além do registro dos seus downloads, conforme a{' '}
                <a className="text-link" href="/docs#privacidade">política de privacidade</a>.
              </p>
            </>
          )}
        </div>
        {error && <Alert variant="destructive" className="mt-6"><AlertDescription>{error}</AlertDescription></Alert>}
      </CardContent>
    </Card>
  );
}
