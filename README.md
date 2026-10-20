# AnyCaptions

Aplicativo desktop para Windows 11 x64, desenvolvido com Rust, Tauri v2 e Vanilla TypeScript.

Esta entrega contém a fundação e o overlay em modo edição: janela transparente, sem moldura, sempre no topo, com arraste, redimensionamento e preferências de aparência em memória. A captura começa parada. Áudio, inferência, bandeja, atalho, click-through e persistência serão implementados nas próximas etapas; iniciar e bloquear ficam indisponíveis até seus requisitos existirem. O botão de sair encerra a aplicação nesta etapa.

## Executar

Pré-requisitos: Rust estável com target `x86_64-pc-windows-msvc` (mínimo 1.90), ferramentas C++ do Visual Studio com Windows SDK, WebView2 e Node 24.15+ da linha 24 (ou 26+).

Na raiz do repositório:

```powershell
npm.cmd --prefix frontend ci
npm.cmd --prefix frontend run tauri -- dev
```

O primeiro comando instala as versões do `package-lock.json`. O segundo inicia o Vite local e a janela Tauri. A CLI é uma dependência local e trabalha em `backend/`; não é preciso instalar `cargo-tauri` globalmente. A primeira compilação baixa as crates do `Cargo.lock` e pode levar mais tempo.

Para gerar apenas o executável, sem instalador:

```powershell
npm.cmd --prefix frontend run tauri -- build --no-bundle
```

Saída: `backend/target/release/anycaptions.exe`. Modelos e ferramentas de IA/Vulkan não são necessários para esta etapa.

## Verificar

```powershell
cargo test --manifest-path backend/Cargo.toml
cargo fmt --manifest-path backend/Cargo.toml -- --check
cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings
npm.cmd --prefix frontend run test -- --run
npm.cmd --prefix frontend run typecheck
npm.cmd --prefix frontend run build
```

`cargo test` compila e executa os testes Rust. Vitest verifica os controles no DOM simulado; `typecheck` verifica os tipos TypeScript; `build` gera os arquivos da interface. Os testes usam recursos simulados, sem microfone, áudio, modelos ou janela nativa.

Na janela real, conferir transparência, permanência no topo, arraste pelo nome/ícone e redimensionamento pelo canto inferior direito. Botões e seletores devem responder sem mover a janela. O tamanho inicial é 800 × 220 e o mínimo é 320 × 160 pixels lógicos, limitados à área útil do monitor. Configurações abre o painel de aparência; Escape retorna à prévia. A janela inicia sem pedir foco.

## Organização e dependências

- `backend/src/domain.rs`: modos, estados, validação e identidade de sessão, sem chamadas à UI ou à IA.
- `backend/src/ipc.rs`: comandos Tauri, eventos tipados e estado protegido por `Mutex`. Falhas retornam códigos, sem conteúdo de falas.
- `backend/src/window.rs`: posição inicial, tamanho, navegação local e criação da janela.
- `frontend/src/overlay.ts`: controles e renderização a partir do estado confirmado pelo Rust. Respostas antigas são ignoradas pela revisão.
- `frontend/src/bridge.ts`: fronteira IPC, substituída nos testes.

Tauri fornece a janela e o IPC; Serde converte os tipos Rust para os dados usados pelo frontend. Vite executa e empacota a interface, TypeScript verifica tipos, Vitest e jsdom testam comportamento DOM. Os testes de IPC usam o runtime simulado do próprio Tauri, incluindo a configuração real de permissões. As dependências de IA e os plugins de bandeja/atalho ainda não são necessários.

Os comandos próprios entram explicitamente na ACL por `AppManifest::commands`. A capability concede somente os controles usados pela janela `overlay`, escuta de eventos, arraste, redimensionamento e fechamento. Não concede emissão de eventos pelo frontend, shell, acesso a arquivos ou origens remotas. A CSP de produção aceita recursos locais e o IPC; a de desenvolvimento acrescenta apenas o Vite em `127.0.0.1:1420` e estilos de desenvolvimento. Navegação externa e novas janelas são bloqueadas no Rust.

Referências: [pré-requisitos Tauri](https://v2.tauri.app/start/prerequisites/), [capabilities e comandos próprios](https://v2.tauri.app/security/capabilities/), [CSP](https://v2.tauri.app/security/csp/), [Vite](https://vite.dev/guide/) e [Vitest](https://vitest.dev/guide/).
