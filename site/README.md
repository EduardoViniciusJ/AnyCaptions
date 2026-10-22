# Site AnyCaptions

`site/backend` contém a API .NET 10, EF Core e PostgreSQL. `site/frontend` contém o site Astro. O aplicativo desktop continua nas pastas `backend` e `frontend` da raiz.

## Segurança e rotas

O site e a API usam o mesmo domínio. O Traefik envia `/api/*` para a API e as demais rotas para o Astro. A API não publica uma porta do container no host. `GET /api/releases/latest` e `GET /api/releases` são públicos; não existe endpoint para listar usuários. `GET /api/auth/me` retorna somente a conta da sessão. O cookie é `HttpOnly`, `Secure`, `SameSite=Lax` e tem prefixo `__Host-`. A API exige o `Origin` do site em `POST /api/auth/logout`. Os dados de áudio do desktop não passam por esta API.

O middleware OAuth recebe os callbacks em `/api/auth/callback/google`, `/api/auth/callback/github` e `/api/auth/callback/discord`. Configure essas URLs nos painéis dos provedores. O estado e o cookie de correlação do middleware protegem o fluxo. Não há senhas nem tokens OAuth persistidos. As chaves que permitem ler cookies entre reinícios ficam no PostgreSQL. Configure acesso restrito ao banco e backups seguros.

## Desenvolvimento

Crie um PostgreSQL local, defina `DATABASE_URL` e as credenciais OAuth como variáveis de ambiente e rode:

```powershell
dotnet run --project site/backend/src/AnyCaptions.Api --urls http://localhost:5080
npm --prefix site/frontend ci
npm --prefix site/frontend run dev
```

O Vite encaminha `/api` para a API local. O login real requer HTTPS para o cookie seguro; use um proxy HTTPS local e registre os callbacks do domínio local junto aos provedores. Sem credenciais OAuth, o site e a API pública continuam funcionando, mas o login responde 503.

## Publicar no Dokploy

Copie `site/.env.example` para as variáveis protegidas do Dokploy. Preencha `SITE_DOMAIN`, `DATABASE_URL`, os segredos OAuth e `TRUSTED_PROXY_NETWORK` com o CIDR real da rede Docker que contém o Traefik. O valor de exemplo do CIDR não deve ser usado sem conferência. O `site/docker-compose.yml` usa a rede externa `dokploy-network`; ajuste seu nome se a instalação usar outro. Configure TLS/certificado para o domínio no Dokploy. A API executa as migrations ao iniciar. Não é necessário disponibilizar PostgreSQL publicamente.

O cadastro de `Release` é feito por manutenção do banco, pois esta primeira versão não oferece painel administrativo. Insira uma versão somente depois de publicar o arquivo. `WindowsInstallerUrl` deve apontar para HTTPS, `Sha256` deve ser o hash do instalador e `IsActive` controla sua visibilidade. O site só oferece a edição padrão enquanto não houver um pacote real com tradução.

Validação local: `dotnet build site/backend/AnyCaptions.sln`, `npm --prefix site/frontend run check`, `npm --prefix site/frontend run build` e `docker compose --env-file site/.env -f site/docker-compose.yml config`.
