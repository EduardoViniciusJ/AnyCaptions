# AnyCaptions

Aplicativo desktop para Windows 11 x64, desenvolvido com Rust, Tauri v2 e Vanilla TypeScript.

O overlay abre em modo edição, sem captura. A bandeja oferece mostrar/editar, iniciar/parar, bloquear/desbloquear e sair. O atalho global inicial é `Ctrl+Alt+L` e pode ser alterado para outra letra no painel. O bloqueio torna a janela transparente e permite que os cliques atravessem a área da legenda; a bandeja permanece disponível para recuperar os controles. Fechar a janela a recolhe para a bandeja. Áudio e modelos ainda não estão integrados, por isso iniciar/parar permanece indisponível.

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

Na janela real, conferir transparência, permanência no topo, arraste pelo nome/ícone e redimensionamento pelo canto inferior direito. Botões e seletores devem responder sem mover a janela. O tamanho inicial é 800 × 220 e o mínimo é 320 × 160 pixels lógicos, limitados à área útil do monitor. Configurações abre aparência e atalho; Escape retorna à prévia. Em `tauri dev`, o botão **Simular legendas** permite verificar texto parcial/final e blocos em português sem IA. Confira também bloqueio com clique atravessando, recuperação pela bandeja/atalho, fechamento para a bandeja e restauração de posição após reiniciar ou desconectar um monitor. A janela inicia sem pedir foco.

Modo, aparência, atalho, identificador da saída escolhida futuramente e geometria ficam em `preferences.json` no diretório de configuração do aplicativo (`%APPDATA%\com.anycaptions.app` no Windows). Arquivo ausente ou inválido usa os valores padrão. Áudio e texto das legendas não são persistidos.

## Organização e dependências

- `backend/src/domain.rs`: modos, estados, validação e identidade de sessão, sem chamadas à UI ou à IA.
- `backend/src/ipc.rs`: comandos Tauri, eventos tipados e estado protegido por `Mutex`. Falhas retornam códigos, sem conteúdo de falas.
- `backend/src/window.rs`: posição inicial, tamanho, navegação local e criação da janela.
- `backend/src/recovery.rs`: bandeja, atalho, recolhimento e saída.
- `backend/src/preferences.rs`: leitura e escrita das preferências locais.
- `frontend/src/overlay.ts`: controles e renderização a partir do estado confirmado pelo Rust. Respostas antigas são ignoradas pela revisão.
- `frontend/src/captions.ts`: texto parcial/final, ordem de segmentos, blocos e tempo de permanência.
- `frontend/src/bridge.ts`: fronteira IPC, substituída nos testes.

Tauri fornece a janela, a bandeja e o IPC; o plugin oficial `tauri-plugin-global-shortcut` registra o atalho no Rust, sem conceder comandos desse plugin à interface. Serde converte os tipos Rust para os dados usados pelo frontend. Vite executa e empacota a interface, TypeScript verifica tipos, Vitest e jsdom testam comportamento DOM. Os testes de IPC usam o runtime simulado do próprio Tauri, incluindo a configuração real de permissões. Dependências de IA ainda não são necessárias.

Os comandos próprios entram explicitamente na ACL por `AppManifest::commands`. A capability concede somente os controles usados pela janela `overlay`, escuta de eventos, arraste, redimensionamento e fechamento. Não concede emissão de eventos pelo frontend, shell, acesso a arquivos ou origens remotas. A CSP de produção aceita recursos locais e o IPC; a de desenvolvimento acrescenta apenas o Vite em `127.0.0.1:1420` e estilos de desenvolvimento. Navegação externa e novas janelas são bloqueadas no Rust.

Referências: [pré-requisitos Tauri](https://v2.tauri.app/start/prerequisites/), [bandeja](https://v2.tauri.app/learn/system-tray/), [atalho global](https://v2.tauri.app/plugin/global-shortcut/), [capabilities e comandos próprios](https://v2.tauri.app/security/capabilities/), [CSP](https://v2.tauri.app/security/csp/), [Vite](https://vite.dev/guide/) e [Vitest](https://vitest.dev/guide/).
