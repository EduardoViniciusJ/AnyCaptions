# Frontend do aplicativo

React + TypeScript, Vite, Tailwind CSS e DaisyUI. O backend Tauri/Rust e os contratos de IPC são independentes da biblioteca de interface.

- `src/app/App.tsx`: composição da janela e ações principais.
- `src/app/useApplication.ts`: snapshots confirmados, revisões, comandos e erros.
- `src/components/`: configurações, aparência, idiomas, downloads e indicadores.
- `src/captions.ts`: estado e paginação do streaming; emite frames para React.
- `src/dictation.ts`: estado, cronômetro e sinais sonoros; emite frames para React.
- `src/caption-layout.ts`: medição no WebView e quebra de linhas.
- `src/contracts.ts` / `src/bridge.ts`: comunicação tipada com Rust.
- `src/i18n/`: textos traduzidos.

React renderiza a interface. Os motores de legendas e ditado não criam nem substituem elementos visíveis. O nó invisível de medição é removido ao desmontar a janela.

O tema DaisyUI é aplicado à `.overlay`. O documento deve permanecer transparente para o modo bloqueado do Tauri. Configurações editáveis ficam em rascunhos locais até Salvar; áudio e idiomas continuam dependendo de confirmação do backend. Eventos de legenda atualizam somente o componente de legenda. Inscrições assíncronas devem liberar listeners mesmo quando a janela é desmontada antes de a inscrição terminar.

Validação, a partir da raiz:

```powershell
npm.cmd --prefix frontend run typecheck
npm.cmd --prefix frontend test -- --run
npm.cmd --prefix frontend run build
```

## Publishing a release

Release notes are always written in English. They are shown on GitHub and on the site's changelog.

1. Bump the version in `backend/Cargo.toml`, `backend/tauri.conf.json` and `frontend/package.json`.
2. Build the installer: `npm run tauri -- build`.
3. Build the self-update assets: `node scripts/package-update.mjs`. The signing key is read from `ANYCAPTIONS_UPDATE_KEY` or `~/.anycaptions/updater.key` and never enters the repository.
4. Create the GitHub release `vX.Y.Z` with the installer and everything in `backend/target/release/update-package/`.
5. Add the version to the site's `Releases` table (see `site/README.md`).

Installed apps verify `update.json` against the public key in `backend/src/updater.rs`. Losing the private key means installed apps can no longer update by themselves, so keep a backup. The package contains only the executable and its DLLs, so models are never downloaded again.
