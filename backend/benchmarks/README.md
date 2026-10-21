# Experimento Whisper

Preparação da T05.1, independente da captura/VAD em desenvolvimento na etapa 4.
O exemplo não habilita transcrição no aplicativo e não escolhe o modelo definitivo.

## Pré-requisitos

Windows x64/MSVC, Rust do projeto, CMake e `libclang.dll`. O script procura o
CMake do Visual Studio quando não está no PATH; configure `CMAKE` se necessário.
Configure `LIBCLANG_PATH` para a pasta que contém a DLL do LLVM. Essas variáveis
são do terminal atual, não precisam de alteração global.

`whisper-rs = 0.16.0` usa `whisper-rs-sys = 0.15.0` e `whisper.cpp 1.8.3` no
lockfile desta entrega. Os bindings prontos dessa versão falharam com layouts C
incompatíveis no MSVC; não usar `WHISPER_DONT_GENERATE_BINDINGS=1`.
O script remove essa variável durante a compilação e a restaura ao terminar.

Vulkan exige adicionalmente SDK com `glslc.exe`, `VULKAN_SDK` e geração de
bindings com libclang. Driver reconhecido por `vulkaninfo` não substitui o SDK.
Não há fallback automático de um experimento Vulkan para CPU.

## Preparar entradas explicitamente

Execute da raiz deste worktree. Os downloads abaixo são preparação manual do
experimento; o aplicativo e o executor de benchmark não fazem downloads.
Não usar gravações de chamadas privadas. Modelos e WAVs ficam ignorados no Git.

```powershell
New-Item -ItemType Directory -Force backend/models, backend/benchmarks/local
curl.exe -fL https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-tiny.en-q5_1.bin -o backend/models/ggml-tiny.en-q5_1.bin
curl.exe -fL https://raw.githubusercontent.com/ggml-org/whisper.cpp/2eeeba56e9edd762b4b38467bab96c2517163158/samples/jfk.wav -o backend/benchmarks/local/jfk.wav
```

O perfil `whisper.example.json` registra modelo, revisão, origem, licença,
SHA-256, referência e parâmetros. Caminhos são relativos ao JSON, não ao terminal.
O executor verifica todos os hashes antes de carregar o modelo. WAV deve ser PCM
16 bits, mono, 16 kHz, entre 1 e 30 segundos; a normalização não faz parte da medição.

Para comparar os demais modelos, baixe os arquivos da mesma revisão e copie o
perfil para `backend/benchmarks/local/`, ajustando caminhos, ID e checksum. Ou
gere os três perfis com fala e três segundos de silêncio sintético:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File backend/scripts/prepare-whisper-profiles.ps1
```

Esse comando recria `local/tiny.en.json`, `local/tiny.en-q5_1.json`,
`local/base.en.json` e `local/silence.wav`; não baixa modelos. Não altere
referência, parâmetros ou fixtures entre os candidatos. Os hashes abaixo são os
OIDs SHA-256 LFS publicados pelo repositório:

| Modelo/arquivo | SHA-256 |
| --- | --- |
| `ggml-tiny.en.bin` | `921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f` |
| `ggml-tiny.en-q5_1.bin` | `c77c5766f1cef09b6b7d47f21b546cbddd4157886b3b5d6d4f709e91e66c7c2b` |
| `ggml-base.en.bin` | `a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002` |

Modelos: licença MIT, revisão `5359861c739e955e79d9a303bcbc70fb988958b1` do
[repositório de modelos](https://huggingface.co/ggerganov/whisper.cpp).
Fixture JFK: amostra pública fornecida pelo repositório MIT do whisper.cpp,
revisão `2eeeba56e9edd762b4b38467bab96c2517163158`; não é redistribuída nesta entrega.
A referência de teste é fixa no perfil. Outros fixtures precisam de origem,
licença, revisão, hash e referência revisados antes de serem usados.

## Executar

```powershell
$env:LIBCLANG_PATH = 'C:/Program Files/LLVM/bin'
powershell -NoProfile -ExecutionPolicy Bypass -File backend/scripts/whisper-benchmark.ps1 -Experiment backend/benchmarks/whisper.example.json -Backend cpu -ReportName tiny-q5-cpu
# Somente após configurar e validar o SDK:
powershell -NoProfile -ExecutionPolicy Bypass -File backend/scripts/whisper-benchmark.ps1 -Experiment backend/benchmarks/whisper.example.json -Backend vulkan -ReportName tiny-q5-vulkan
# Se MSBuild falhar por caminhos internos longos ou faltar espaço no disco do worktree:
powershell -NoProfile -ExecutionPolicy Bypass -File backend/scripts/whisper-benchmark.ps1 -Experiment backend/benchmarks/whisper.example.json -Backend vulkan -VulkanTargetDirectory C:/ac-vk -ReportName tiny-q5-vulkan
```

`ExecutionPolicy Bypass` vale apenas para esse processo. O script não instala
ferramentas nem altera configurações globais; limita a compilação a quatro jobs,
usa `backend/target/` deste worktree para CPU e um alvo Vulkan isolado e curto
(por padrão, `ac-vk` na raiz do disco do repositório). `-VulkanTargetDirectory`
permite escolher outro diretório curto quando necessário. O script encerra o
processo se ultrapassar o timeout.
Acrescenta `/O2 /DNDEBUG` a `CMAKE_C_FLAGS_RELEASE`/`CMAKE_CXX_FLAGS_RELEASE` durante a compilação MSVC e registra
essas flags no relatório. Nesta combinação de ferramentas, os comandos C/C++
gerados inicialmente não tinham otimização, mesmo com Cargo/CMake em release.
As variáveis anteriores são restauradas ao terminar.
Um registro em `target/whisper-benchmark-native.json` detecta mudanças nas flags,
ferramentas, lockfile ou backend e invalida apenas o pacote nativo desse target
antes do build. Isso evita reutilizar um motor compilado com opções anteriores.
Execute os benchmarks sequencialmente, sem builds paralelos ou outro trabalho de
CPU/GPU, para evitar distorcer a comparação. O timeout padrão é 600 segundos.

Comando Rust equivalente (com CMake/libclang configurados):

```powershell
$env:CMAKE_C_FLAGS_RELEASE = '/O2 /DNDEBUG'
$env:CMAKE_CXX_FLAGS_RELEASE = '/O2 /DNDEBUG'
cargo run --locked --manifest-path backend/Cargo.toml --release --features whisper-benchmark --example whisper_benchmark -- backend/benchmarks/whisper.example.json cpu
```

O exemplo Rust imprime um JSON de métricas; não mede RAM sozinho. O wrapper
PowerShell acrescenta ambiente e pico de working set observado a cada 100 ms,
incluindo carga e aquecimento. Em Vulkan, o exemplo também registra memória
livre/total do dispositivo antes e após carga, aquecimento e inferências; esse
valor inclui outros processos e não mede VRAM atribuída especificamente ao
AnyCaptions. Salva `backend/benchmarks/local/<ReportName>.json`.
Não salva transcrição, logs nativos ou áudio capturado. A saída é offline depois
que dependências, ferramentas e entradas estiverem disponíveis.

## Como interpretar

- Contexto/modelo e estado são criados uma vez. `load_and_state_ms` não inclui
  leitura dos fixtures e verificação de hashes.
- Cada fixture tem aquecimento separado (`warmup_ms`), excluído dos percentis.
- Inferências medidas reutilizam o estado nativo, com `no_context=true`, origem
  `en`, `translate=false`, greedy/best-of 1, temperatura 0 e sem fallback de
  temperatura. Flash attention fica desabilitado em ambos os backends.
- Tempo medido inclui inferência e extração do texto em memória; exclui cálculo
  de WER. Não é latência de parcial, tempo de segmentação ou latência do overlay.
- P50/P95 usam nearest rank. Cinco repetições servem para um ensaio inicial;
  P95 com cinco amostras é o máximo observado, não estimativa robusta de cauda.
- RTF é tempo de processamento / duração do áudio. RTF abaixo de 1 em um fixture
  isolado não comprova que o pipeline completo acompanha áudio ao vivo.
- WER usa distância de edição por palavra, ignora caixa e separa pontuação,
  preservando apóstrofos. Não normaliza números nem remove repetições. Pode
  superar 1 com inserções. Referência vazia produz WER `null` e conta as palavras
  indevidas, permitindo avaliar silêncio sem divisão por zero.
- `requested_backend=vulkan` identifica o pedido. `gpu_execution_verified=false`
  permanece explícito: compilar Vulkan e listar GPU não prova execução de todos
  os operadores nela. Confirmar atividade e memória da GPU antes de aprovar.
- Pico de VRAM observado é memória ocupada no dispositivo inteiro, não VRAM
  atribuída ao processo. Medição confiável da alocação do processo e disputa com
  jogo precisam de validação independente.
  Medir VRAM dedicada do processo e repetir cena/configuração de jogo com e sem
  Whisper, registrando FPS/frame time e carga externa. Não preencher zero quando
  uma métrica não estiver disponível.

## Validação e sequência

```powershell
cargo test --manifest-path backend/Cargo.toml
cargo fmt --manifest-path backend/Cargo.toml -- --check
cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings
# Verifica também o exemplo opcional com os pré-requisitos nativos configurados:
cargo clippy --manifest-path backend/Cargo.toml --release --features whisper-benchmark --example whisper_benchmark -- -D warnings
```

Os testes rápidos não habilitam Whisper nem carregam modelos. `hound` lê WAV de
fixture e `sha2` verifica integridade; são dependências de desenvolvimento, com
`sha2` reutilizando a linha já presente no lockfile. `whisper-rs` é opcional.
Os testes reais são os comandos explícitos acima, separados da suíte rápida.

T05.1 continua pendente enquanto faltarem Vulkan funcional, VRAM, disputa com
jogo e avaliação representativa. T05.2–T05.4 aguardam essa avaliação e a
integração da etapa 4; não há adaptador de produção nem ligação ao IPC aqui.

Fontes: [whisper-rs 0.16.0](https://docs.rs/whisper-rs/0.16.0/whisper_rs/),
[whisper.cpp](https://github.com/ggml-org/whisper.cpp/tree/v1.8.3),
[hound](https://docs.rs/hound/3.5.1/hound/) e
[RustCrypto SHA-2](https://docs.rs/sha2/0.10.9/sha2/).
