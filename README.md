<p align="center">
  <img src="backend/icons/icon.png" alt="AnyCaptions icon" width="128" height="128" />
</p>

<h1 align="center">AnyCaptions</h1>

<p align="center">
  <strong>Real-time captions and translation for any audio on your PC.</strong><br />
  Fully local speech recognition and translation for Windows — no cloud, no account.
</p>

<p align="center">
  <a href="https://anycaptions.app">Website &amp; download</a>
</p>


<p align="center">
  <a href="#features">Features</a> ·
  <a href="#tech-stack">Tech stack</a> ·
  <a href="#models">Models</a> ·
  <a href="#getting-started">Getting started</a> ·
  <a href="https://github.com/EduardoViniciusJ/AnyCaptions/issues">Report a bug or suggest an idea</a>
</p>

> [!NOTE]
> AnyCaptions is free and still in development. It listens to the audio you are already hearing — games, videos, calls, any app — and shows a floating caption overlay on top of everything. Speech recognition and translation run entirely on your machine: no audio or text ever leaves your computer, and the app works offline once installed.

## Features

- **Live transcription of system audio:** captured via WASAPI loopback — no microphone or virtual cable needed, just pick the output device you already use.
- **Real-time translation into 37 languages:** from Portuguese to Japanese, with automatic detection of the spoken language.
- **Fast English ↔ Portuguese path:** runs on the CPU (~90 ms per sentence), with the full 37-language model on the GPU.
- **Optional speaker labels:** tells apart who is talking with generic labels ("Person 1", "Person 2") that follow the captions, including translated ones. Off by default; the model is a 107 MB in-app download. Labels come from the voices alone — nobody is identified by name.
- **Voice dictation:** speak and your words are typed into whatever application you are using.
- **Transparent, click-through overlay:** when locked, clicks pass straight through the caption; recover it any time from the tray or the global shortcut (`Ctrl+Alt+L`, configurable).
- **In-app model download:** the large translation model is a one-click download with integrity verification; everything else ships in the installer.
- **Customizable captions:** size, colors, width and position.
- **UI in 6 languages:** Portuguese (Brazil), English, Spanish, French, Russian and Simplified Chinese.
- **Private by design:** no history, no telemetry, nothing sent to the internet.

## Shortcuts

| Shortcut | Action |
|---|---|
| `Ctrl+Alt+L` | Lock or recover the caption overlay (default, can be changed) |
| `Ctrl+Space` | Start or stop voice dictation (default, can be changed to F6–F24 or Ctrl/Alt + key) |

## Tech stack

| Layer | Technologies |
|---|---|
| Desktop | Tauri 2 · Rust · WebView2 |
| Interface | Vanilla TypeScript · Vite |
| Audio capture | WASAPI loopback |
| Local inference | NeMo-Speech.cpp · CTranslate2 · GPU (Vulkan) with CPU fallback |
| Website ([`site/`](site/README.md)) | Astro · ASP.NET Core · PostgreSQL |
| Tests | Vitest · Rust tests |
| Code quality | TypeScript strict mode · Clippy · rustfmt |

## Models

All models run locally:

| Model | Purpose | Runs on | Availability |
| --- | --- | --- | --- |
| [NVIDIA Nemotron 3.5 ASR Streaming 0.6B](https://huggingface.co/nvidia/nemotron-3.5-asr-streaming-0.6b) | Real-time speech recognition (punctuation, end-of-sentence and language detection built in) | GPU (Vulkan), CPU fallback | Ships with the installer |
| [NVIDIA Riva Translate 4B Instruct v2](https://huggingface.co/nvidia/Riva-Translate-4B-Instruct-v2) | Translation into 37 languages | GPU | Downloaded inside the app (2.8 GB) |
| [NVIDIA Nemotron 3 Diarization](https://huggingface.co/nvidia/Nemotron-3-Diarization) | Speaker labels (streaming Sortformer, up to 8 generic speaker slots) | GPU (Vulkan), CPU fallback (alongside the ASR model) | Downloaded inside the app (107 MB) |
| [Helsinki-NLP OPUS-MT](https://huggingface.co/Helsinki-NLP/opus-mt-tc-big-en-pt) (en ↔ pt) | Fast English ↔ Portuguese translation | CPU | Ships with the installer |

Inference runs on [NeMo-Speech.cpp](https://github.com/NVIDIA/NeMo-Speech.cpp) (Apache-2.0) for recognition and Riva, and on [CTranslate2](https://github.com/OpenNMT/CTranslate2) (MIT) for OPUS-MT.

**Licenses:** Nemotron ASR and Nemotron 3 Diarization under [OpenMDW-1.1](https://openmdw.ai/license/1-1/); Riva Translate — Licensed by NVIDIA Corporation under the [NVIDIA Open Model License](https://www.nvidia.com/en-us/agreements/enterprise-software/nvidia-open-model-license/); OPUS-MT under [CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/) (credits: Helsinki-NLP/OPUS-MT, Jörg Tiedemann). Selector flags from [flag-icons](https://github.com/lipis/flag-icons) (MIT).

## Getting started

The easiest way to use AnyCaptions is the Windows installer from [anycaptions.app](https://anycaptions.app).

To build from source on Windows 11, install **Rust 1.90+ (MSVC, `x86_64-pc-windows-msvc`)**, the Visual Studio C++ build tools with the Windows SDK, the **WebView2 Runtime**, and **Node.js 24.15 or newer**. Then run:

```powershell
git clone https://github.com/EduardoViniciusJ/AnyCaptions.git
cd AnyCaptions
npm.cmd --prefix frontend ci
npm.cmd --prefix frontend run tauri -- dev
```

Models and the inference libraries are not required to compile — only to transcribe and translate (see `backend/models/README.md`).

<details>
<summary>Development commands</summary>

From the project root:

```powershell
cargo test --manifest-path backend/Cargo.toml
cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings
npm.cmd --prefix frontend run test -- --run
npm.cmd --prefix frontend run typecheck
```

</details>

## Your data

Audio and captions stay in memory and are never written to disk — there is no history. Settings are stored locally in `%APPDATA%\com.anycaptions.app`, and models live in the `models/` folder next to the executable. Nothing is sent to the internet; the only network access is the optional in-app model download.
