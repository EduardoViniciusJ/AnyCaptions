//! Injeção do texto ditado no aplicativo em foco.
//!
//! Técnica: coloca o texto na área de transferência, simula Ctrl+V e restaura
//! o texto anterior. O texto ditado só passa pelo clipboard por um instante;
//! nada é gravado em disco ou logs.

use std::{thread, time::Duration};

use windows::Win32::{
    Foundation::{HANDLE, HGLOBAL},
    System::{
        DataExchange::{
            CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
        },
        Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock},
    },
    UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
        VIRTUAL_KEY, VK_LMENU, VK_LSHIFT, VK_MENU, VK_RMENU, VK_RSHIFT, VK_SHIFT, VK_V,
    },
};

/// CF_UNICODETEXT: formato de texto UTF-16 do clipboard (constante Win32 estável).
const CF_UNICODETEXT: u32 = 13;
/// Maior conteúdo anterior restaurado (textos maiores não são preservados).
const MAX_RESTORED_UTF16: usize = 2 * 1024 * 1024;
/// Tempo para o aplicativo em foco processar o Ctrl+V antes da restauração.
const PASTE_SETTLE: Duration = Duration::from_millis(150);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectError {
    ClipboardUnavailable,
    InjectionFailed,
}

/// Cola `text` no aplicativo em foco e restaura o texto anterior do clipboard.
/// Conteúdo anterior que não for texto (imagem, arquivos) não é preservado.
pub fn paste_into_focused_app(text: &str) -> Result<(), InjectError> {
    let previous = replace_clipboard_text(text)?;
    press_ctrl_v()?;
    thread::sleep(PASTE_SETTLE);
    if let Some(previous) = previous {
        // Falha na restauração não desfaz a colagem já concluída.
        let _ = replace_clipboard_text(&previous);
    }
    Ok(())
}

/// Deixa `text` no clipboard sem colar nem restaurar o conteúdo anterior.
/// Usado quando a tradução do ditado falha: nada é injetado no app em foco,
/// mas o texto original fica disponível para colagem manual.
pub fn copy_to_clipboard(text: &str) -> Result<(), InjectError> {
    replace_clipboard_text(text).map(|_| ())
}

/// Garante `CloseClipboard` em todos os caminhos depois de um `OpenClipboard`.
struct ClipboardGuard;

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        // SAFETY: só construído após OpenClipboard bem-sucedido nesta thread.
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

/// Troca o texto do clipboard, devolvendo o texto anterior quando havia um.
fn replace_clipboard_text(text: &str) -> Result<Option<String>, InjectError> {
    let _guard = open_clipboard_with_retry()?;
    // SAFETY: clipboard aberto por esta thread; handles verificados antes do uso.
    unsafe {
        let previous = read_clipboard_text();
        EmptyClipboard().map_err(|_| InjectError::ClipboardUnavailable)?;
        let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = utf16.len() * 2;
        let memory =
            GlobalAlloc(GMEM_MOVEABLE, bytes).map_err(|_| InjectError::ClipboardUnavailable)?;
        let target = GlobalLock(memory);
        if target.is_null() {
            return Err(InjectError::ClipboardUnavailable);
        }
        std::ptr::copy_nonoverlapping(utf16.as_ptr(), target.cast::<u16>(), utf16.len());
        let _ = GlobalUnlock(memory);
        // Com sucesso, o sistema passa a ser dono da memória; sem sucesso ela
        // vaza uma vez (liberá-la aqui arrisca double-free em corridas raras).
        SetClipboardData(CF_UNICODETEXT, Some(HANDLE(memory.0)))
            .map_err(|_| InjectError::ClipboardUnavailable)?;
        Ok(previous)
    }
}

fn open_clipboard_with_retry() -> Result<ClipboardGuard, InjectError> {
    // Outro processo pode estar com o clipboard aberto; tentar brevemente.
    for _ in 0..10 {
        // SAFETY: chamada direta documentada; None associa à thread atual.
        if unsafe { OpenClipboard(None) }.is_ok() {
            return Ok(ClipboardGuard);
        }
        thread::sleep(Duration::from_millis(15));
    }
    Err(InjectError::ClipboardUnavailable)
}

/// Lê o texto atual do clipboard. Requer o clipboard aberto.
unsafe fn read_clipboard_text() -> Option<String> {
    // SAFETY (função toda): handle validado, leitura limitada por GlobalSize.
    unsafe {
        let handle = GetClipboardData(CF_UNICODETEXT).ok()?;
        let memory = HGLOBAL(handle.0);
        let size_units = GlobalSize(memory) / 2;
        if size_units == 0 || size_units > MAX_RESTORED_UTF16 {
            return None;
        }
        let source = GlobalLock(memory).cast::<u16>();
        if source.is_null() {
            return None;
        }
        let units = std::slice::from_raw_parts(source, size_units);
        let length = units.iter().position(|unit| *unit == 0).unwrap_or(0);
        let text = String::from_utf16_lossy(&units[..length]);
        let _ = GlobalUnlock(memory);
        Some(text)
    }
}

fn key_event(key: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: 0,
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    KEYBD_EVENT_FLAGS(0)
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn press_ctrl_v() -> Result<(), InjectError> {
    use windows::Win32::UI::Input::KeyboardAndMouse::VK_CONTROL;
    // O usuário pode ainda estar segurando Alt/Shift do próprio atalho do
    // ditado; soltar esses modificadores evita colar como Ctrl+Alt+V.
    let inputs = [
        key_event(VK_MENU, true),
        key_event(VK_LMENU, true),
        key_event(VK_RMENU, true),
        key_event(VK_SHIFT, true),
        key_event(VK_LSHIFT, true),
        key_event(VK_RSHIFT, true),
        key_event(VK_CONTROL, false),
        key_event(VK_V, false),
        key_event(VK_V, true),
        key_event(VK_CONTROL, true),
    ];
    // SAFETY: array local com cbSize correto; SendInput devolve quantos entraram.
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent == inputs.len() as u32 {
        Ok(())
    } else {
        Err(InjectError::InjectionFailed)
    }
}
