# Modelos

Arquivos de modelo (GGUF) organizados por capacidade. Eles não são versionados;
a fonte da verdade sobre cada modelo (origem, revisão, licença e SHA-256) é o
catálogo em `backend/src/models/catalog.rs`, que verifica o arquivo antes de carregar.

```
models/
  asr/            # transcrição de fala
    <modelo>/     # variantes de quantização compartilham a pasta
  translation/    # tradução de texto
  diarization/    # identificação de falantes (rótulos genéricos "Pessoa N")
```

O modelo de diarização (`diarization/nemotron-3-diarization/Nemotron-3-Diarization.q8_0.gguf`)
é opcional: baixado dentro do app pelas configurações ("Identificação de
falantes"), direto do repositório da NVIDIA na revisão fixada no catálogo.
Sem ele, a transcrição funciona normalmente, sem rótulos.

A raiz pode ser trocada com a variável de ambiente `ANYCAPTIONS_MODELS_DIR`.
