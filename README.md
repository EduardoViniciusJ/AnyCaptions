# AnyCaptions

**Real-time captions and translation for any audio on your Windows PC — fully local, no cloud.**

**Website & download:** [anycaptions.app](https://anycaptions.app)

AnyCaptions listens to the audio you are already hearing — games, videos, calls, any app — and shows a floating caption overlay on top of everything. Speech recognition and translation run entirely on your machine: no audio or text ever leaves your computer, and the app works offline once installed.

## Features

- **Live transcription of system audio** via WASAPI loopback — no microphone or virtual cable needed, just pick the output device you already use
- **Real-time translation into 37 languages**, from Portuguese to Japanese, with automatic detection of the spoken language
- **Fast English ↔ Portuguese path** on the CPU (~90 ms per sentence), with the full 37-language model on the GPU
- **Voice dictation** — speak and your words are typed into whatever application you are using
- **Transparent, click-through overlay** — when locked, clicks pass straight through the caption; recover it any time from the tray or the global shortcut (`Ctrl+Alt+L`, configurable)
- **In-app model download** — the large translation model is a one-click download with integrity verification; everything else ships in the installer
- **Customizable captions**: size, colors, width and position
- **UI in 6 languages**: Portuguese (Brazil), English, Spanish, French, Russian and Simplified Chinese
- **Private by design**: no history, no telemetry, nothing sent to the internet

## Tech stack

- **Desktop app** (`backend/` + `frontend/`): Rust + Tauri v2, Vanilla TypeScript + Vite
- **Website** ([`site/`](site/README.md)): Astro frontend + ASP.NET Core API + PostgreSQL

## Models

All models run locally:

| Model | Purpose | Runs on | Availability |
| --- | --- | --- | --- |
| [NVIDIA Nemotron 3.5 ASR Streaming 0.6B](https://huggingface.co/nvidia/nemotron-3.5-asr-streaming-0.6b) | Real-time speech recognition (punctuation, end-of-sentence and language detection built in) | GPU (Vulkan), CPU fallback | Ships with the installer |
| [NVIDIA Riva Translate 4B Instruct v2](https://huggingface.co/nvidia/Riva-Translate-4B-Instruct-v2) | Translation into 37 languages | GPU | Downloaded inside the app (2.8 GB) |
| [Helsinki-NLP OPUS-MT](https://huggingface.co/Helsinki-NLP/opus-mt-tc-big-en-pt) (en ↔ pt) | Fast English ↔ Portuguese translation | CPU | Ships with the installer |

Inference runs on [NeMo-Speech.cpp](https://github.com/NVIDIA/NeMo-Speech.cpp) (Apache-2.0) for recognition and Riva, and on [CTranslate2](https://github.com/OpenNMT/CTranslate2) (MIT) for OPUS-MT.

**Licenses:** Nemotron under [OpenMDW-1.1](https://openmdw.ai/license/1-1/); Riva Translate — Licensed by NVIDIA Corporation under the [NVIDIA Open Model License](https://www.nvidia.com/en-us/agreements/enterprise-software/nvidia-open-model-license/); OPUS-MT under [CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/) (credits: Helsinki-NLP/OPUS-MT, Jörg Tiedemann). Selector flags from [flag-icons](https://github.com/lipis/flag-icons) (MIT).

## Building from source

Prerequisites: stable Rust (`x86_64-pc-windows-msvc`, 1.90+), Visual Studio C++ tools with the Windows SDK, WebView2 and Node 24.15+.

```powershell
npm.cmd --prefix frontend ci
npm.cmd --prefix frontend run tauri -- dev
```

Models and the inference libraries are not required to compile — only to transcribe and translate (see `backend/models/README.md`).

### Checks

```powershell
cargo test --manifest-path backend/Cargo.toml
cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings
npm.cmd --prefix frontend run test -- --run
npm.cmd --prefix frontend run typecheck
```
