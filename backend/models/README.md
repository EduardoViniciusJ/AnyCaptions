# Modelos

Arquivos de modelo (GGUF) organizados por capacidade. Eles não são versionados;
a fonte da verdade sobre cada modelo (origem, revisão, licença e SHA-256) é o
catálogo em `backend/src/models/catalog.rs`, que verifica o arquivo antes de carregar.

```
models/
  asr/            # transcrição de fala
    <modelo>/     # variantes de quantização compartilham a pasta
  translation/    # tradução de texto
```

A raiz pode ser trocada com a variável de ambiente `ANYCAPTIONS_MODELS_DIR`.
