# AnyCaptions

Aplicativo desktop para Windows 11 x64, desenvolvido com Rust, Tauri v2 e Vanilla TypeScript.

O overlay abre em modo edição, sem captura. A bandeja oferece mostrar/editar, iniciar/parar, bloquear/desbloquear e sair. O atalho global inicial é `Ctrl+Alt+L` e pode ser alterado para outra letra no painel. O bloqueio torna a janela transparente e permite que os cliques atravessem a área da legenda; a bandeja permanece disponível para recuperar os controles. Fechar a janela a recolhe para a bandeja. O modo Auto captura a saída selecionada e transcreve localmente, no idioma falado, com o NVIDIA Nemotron 3.5 ASR Streaming executado pelo NeMo-Speech.cpp. EN → PT permanece desabilitado até a integração de um tradutor local.

## Transcrição local

O reconhecimento usa dois itens locais, ignorados pelo Git:

- Modelo `nemotron-3.5-asr-streaming-0.6b.q8_0.gguf` (742.090.464 bytes, SHA-256 `3fc991d3badad7277c11030a7519832cddaf2057aafed6d4b25147e953a070b1`, revisão `ea30d66debe3740a08b573244286791d423d6b3e` de [nvidia/nemotron-3.5-asr-streaming-0.6b](https://huggingface.co/nvidia/nemotron-3.5-asr-streaming-0.6b), licença [OpenMDW-1.1](https://openmdw.ai/license/1-1/)) em `backend/models/` ou na pasta de `ANYCAPTIONS_MODELS_DIR`. O arquivo é verificado por SHA-256 antes de carregar.
- Biblioteca [NeMo-Speech.cpp](https://github.com/NVIDIA/NeMo-Speech.cpp) v0.2.0 (Apache-2.0), compilada com Vulkan em `backend/local-data/nemo-speech/build-vulkan-asr/bin`, na pasta de `ANYCAPTIONS_NEMO_DIR` ou ao lado do executável. Ela é carregada em tempo de execução (`nemo_speech_asr_c.dll` e dependências); sem ela o app abre normalmente e informa que a transcrição está indisponível.

O modelo roda na GPU pelo Vulkan e volta para a CPU se nenhum dispositivo puder ser usado. Ele mesmo detecta voz, fim de frase, pontuação e idioma, sem VAD ou segmentador externo.

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

Saída: `backend/target/release/anycaptions.exe`. O modelo e a biblioteca NeMo não são necessários para compilar; só para transcrever.

## Verificar

```powershell
cargo test --manifest-path backend/Cargo.toml
cargo fmt --manifest-path backend/Cargo.toml -- --check
cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings
npm.cmd --prefix frontend run test -- --run
npm.cmd --prefix frontend run typecheck
npm.cmd --prefix frontend run build
```

`cargo test --manifest-path backend/Cargo.toml` compila e executa os testes rápidos Rust. A integração real é separada: `cargo test --manifest-path backend/Cargo.toml --test real_nemotron -- --ignored`, com o modelo e a biblioteca NeMo descritos acima e o fixture `jfk.wav` em `backend/benchmarks/local/`. Vitest verifica os controles no DOM simulado; `typecheck` verifica os tipos TypeScript; `build` gera os arquivos da interface.

Na janela real, conferir transparência, permanência no topo, arraste pelo nome/ícone e redimensionamento pelo canto inferior direito. Botões e seletores devem responder sem mover a janela. O tamanho inicial é 800 × 360 e o mínimo é 320 × 160 pixels lógicos, limitados à área útil do monitor. Ao abrir configurações, a janela cresce até pelo menos 640 × 360, se houver espaço. Aparência e saída são aplicadas ao alterar; Salvar confirma o atalho e retorna à tela principal. Escape também retorna. Confira bloqueio com clique atravessando, recuperação pela bandeja/atalho e restauração de posição após reiniciar ou desconectar um monitor. A janela inicia sem pedir foco.

O modo de transcrição mostra somente o trecho atual, com até duas linhas e 42 caracteres por linha (menos em janelas estreitas). Revisões substituem o texto; um trecho novo substitui imediatamente o anterior. O frontend acompanha a frase/bloco mais recente da revisão cumulativa, sem reproduzir uma fila de frases antigas. Uma final fica visível por 1,2–3 s, a 22 caracteres/s; uma parcial sem novidades desaparece em 3 s. O Nemotron processa o áudio em passos de 160 ms e emite parciais enquanto a pessoa fala e uma final quando detecta o fim da frase. Esses intervalos não incluem o tempo de inferência e não garantem sincronização palavra por palavra.

Modo, aparência, atalho, identificador da saída escolhida futuramente e geometria ficam em `preferences.json` no diretório de configuração do aplicativo (`%APPDATA%\com.anycaptions.app` no Windows). Arquivo ausente ou inválido usa os valores padrão. Áudio e texto das legendas não são persistidos.

## Organização e dependências

- `backend/src/domain.rs`: modos, estados, validação e identidade de sessão, sem chamadas à UI ou à IA.
- `backend/src/ipc.rs`: comandos Tauri, eventos tipados e estado protegido por `Mutex`. Falhas retornam códigos, sem conteúdo de falas.
- `backend/src/window.rs`: posição inicial, tamanho, navegação local e criação da janela.
- `backend/src/recovery.rs`: bandeja, atalho, recolhimento e saída.
- `backend/src/preferences.rs`: leitura e escrita das preferências locais.
- `backend/src/audio/`: captura WASAPI loopback, conversão para mono e reamostragem para 16 kHz.
- `backend/src/pipeline/`: worker com filas limitadas, cancelamento e identidade de sessão.
- `backend/src/transcription/`: `nemotron.rs` (FFI com a API C do NeMo-Speech.cpp), `streaming.rs` (resultados parciais/finais em legendas), `session.rs` (liga captura, pipeline e eventos) e `profile.rs` (verificação SHA-256 do modelo).
- `frontend/src/overlay.ts`: controles e renderização a partir do estado confirmado pelo Rust. Respostas antigas são ignoradas pela revisão.
- `frontend/src/captions.ts`: texto parcial/final, ordem de segmentos, blocos e tempo de permanência.
- `frontend/src/bridge.ts`: fronteira IPC, substituída nos testes.

Tauri fornece a janela, a bandeja e o IPC; o plugin oficial `tauri-plugin-global-shortcut` registra o atalho no Rust, sem conceder comandos desse plugin à interface. Serde converte os tipos Rust para os dados usados pelo frontend. Vite executa e empacota a interface, TypeScript verifica tipos, Vitest e jsdom testam comportamento DOM. Os testes de IPC usam o runtime simulado do próprio Tauri, incluindo a configuração real de permissões. `wasapi` captura o áudio, `rubato` reamostra, `sha2` verifica o modelo e `libloading` carrega a biblioteca NeMo em tempo de execução, então os testes rápidos não precisam do modelo nem da GPU.

Os comandos próprios entram explicitamente na ACL por `AppManifest::commands`. A capability concede somente os controles usados pela janela `overlay`, escuta de eventos, arraste, redimensionamento e fechamento. Não concede emissão de eventos pelo frontend, shell, acesso a arquivos ou origens remotas. A CSP de produção aceita recursos locais e o IPC; a de desenvolvimento acrescenta apenas o Vite em `127.0.0.1:1420` e estilos de desenvolvimento. Navegação externa e novas janelas são bloqueadas no Rust.

Referências: [pré-requisitos Tauri](https://v2.tauri.app/start/prerequisites/), [bandeja](https://v2.tauri.app/learn/system-tray/), [atalho global](https://v2.tauri.app/plugin/global-shortcut/), [capabilities e comandos próprios](https://v2.tauri.app/security/capabilities/), [CSP](https://v2.tauri.app/security/csp/), [Vite](https://vite.dev/guide/) e [Vitest](https://vitest.dev/guide/).
