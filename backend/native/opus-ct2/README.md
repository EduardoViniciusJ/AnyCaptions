# opus-ct2

Source of `opus_ct2_c.dll`, the small C library the app loads at runtime for
its two CPU translation engines:

- **OPUS-MT** (`opus_ct2_translate`): one model per direction.
- **M2M100** (`opus_ct2_translate_to`): one model for every language pair.

It wraps [CTranslate2](https://github.com/OpenNMT/CTranslate2) (MIT) and
[SentencePiece](https://github.com/google/sentencepiece) (Apache-2.0) behind a
plain C ABI (`opus_ct2_c.h`): UTF-8 text in, translated UTF-8 text out. The
Rust side is `backend/src/translation/ct2.rs`.

## Build (Windows, Visual Studio with CMake)

The two libraries are cloned next to this folder's build, at pinned releases:

```
git clone --depth 1 --branch v4.9.0 --recurse-submodules --shallow-submodules https://github.com/OpenNMT/CTranslate2.git deps/CTranslate2
git clone --depth 1 --branch v0.2.3 https://github.com/google/sentencepiece.git deps/sentencepiece

cmake -S . -B build -A x64 -DCMAKE_POLICY_VERSION_MINIMUM=3.5
cmake --build build --config Release --target opus_ct2_c
```

`deps/` and `build/` are not versioned. Use `-DOPUS_CT2_DEPS_DIR=<folder>` to
keep the checkouts somewhere else.

The build produces `opus_ct2_c.dll` and `ctranslate2.dll`. Both go in the
app's native library folder (the same one as the NeMo-Speech libraries), where
the installer picks them up.

## Notes

- CPU only, int8 through Ruy: no MKL, oneDNN, CUDA or OpenMP runtime to ship.
- Decoding is greedy (beam size 1) and bounded to 256 tokens: live captions
  need latency more than the small quality edge of a wider beam.
- The app treats `opus_ct2_translate_to` as optional, so an older library
  still serves OPUS-MT; M2M100 then reports as unavailable.
