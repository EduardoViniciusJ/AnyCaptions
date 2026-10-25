//! Ditado por voz (push-to-talk): configurações e máquina de estados.
//!
//! O áudio do microfone nunca sai da memória; estas estruturas só guardam
//! preferências e controlam o ciclo ocioso → gravando → finalizando.

pub mod inject;
pub mod machine;
pub mod runtime;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DictationError {
    InvalidShortcut,
    ShortcutConflict,
    InvalidDuration,
    InvalidLanguage,
}

/// Como o atalho controla a gravação.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActivationMode {
    /// Segurar para falar: gravar enquanto a tecla estiver pressionada.
    #[default]
    Hold,
    /// Alternar: um toque inicia, outro finaliza.
    Toggle,
}

pub const MIN_MAX_DURATION_SECS: u16 = 5;
pub const MAX_MAX_DURATION_SECS: u16 = 300;

/// Idiomas aceitos para fixar o reconhecimento do ditado: a lista oficial do
/// modelo ASR, compartilhada com as legendas (`crate::domain::ASR_LANGUAGES`).
pub use crate::domain::ASR_LANGUAGES as DICTATION_LANGUAGES;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DictationSettings {
    pub enabled: bool,
    pub shortcut: String,
    pub activation: ActivationMode,
    pub sound_cue: bool,
    /// Mostrar o texto reconhecido no indicador. Desligado, o indicador fica
    /// compacto (ponto vermelho + tempo); a indicação visual nunca some.
    #[serde(default = "default_true")]
    pub show_live_text: bool,
    /// Aplicar o teto de duração. Desligado, a gravação só termina por ação
    /// do usuário (soltar/alternar/Esc) ou erro.
    #[serde(default = "default_true")]
    pub limit_duration: bool,
    /// Teto de gravação em segundos: proteção contra o modo alternar
    /// esquecido ou uma tecla presa no modo segurar.
    pub max_duration_secs: u16,
    pub input_id: Option<String>,
    /// Idioma fixo do reconhecimento (`None` = detecção automática). Fixar o
    /// idioma condiciona o decodificador e melhora a precisão na fala nele.
    #[serde(default)]
    pub language: Option<String>,
}

fn default_true() -> bool {
    true
}

impl Default for DictationSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            shortcut: "Ctrl+Space".into(),
            activation: ActivationMode::Hold,
            sound_cue: true,
            show_live_text: true,
            limit_duration: true,
            max_duration_secs: 60,
            input_id: None,
            language: None,
        }
    }
}

impl DictationSettings {
    /// Valida as configurações; `overlay_shortcut` é o atalho de recuperação
    /// do overlay, que não pode coincidir com o do ditado.
    pub fn validate(&self, overlay_shortcut: &str) -> Result<(), DictationError> {
        let shortcut = normalize_shortcut(&self.shortcut)?;
        if shortcut.eq_ignore_ascii_case(overlay_shortcut.trim()) {
            return Err(DictationError::ShortcutConflict);
        }
        if !(MIN_MAX_DURATION_SECS..=MAX_MAX_DURATION_SECS).contains(&self.max_duration_secs) {
            return Err(DictationError::InvalidDuration);
        }
        if self
            .input_id
            .as_ref()
            .is_some_and(|id| id.is_empty() || id.len() > 512)
        {
            return Err(DictationError::InvalidShortcut);
        }
        if self
            .language
            .as_ref()
            .is_some_and(|code| !DICTATION_LANGUAGES.contains(&code.as_str()))
        {
            return Err(DictationError::InvalidLanguage);
        }
        Ok(())
    }
}

/// Normaliza o atalho do ditado para a forma canônica aceita pelo plugin de
/// atalhos globais. Formas válidas: uma tecla de função F6–F24 sozinha, ou
/// pelo menos um modificador (Ctrl/Alt/Shift, nessa ordem) com uma tecla
/// A–Z, 0–9 ou Space. Teclas de digitação puras são recusadas: o atalho é
/// global e roubaria a tecla de todos os aplicativos.
pub fn normalize_shortcut(value: &str) -> Result<String, DictationError> {
    let pieces: Vec<_> = value.split('+').map(str::trim).collect();
    if pieces.iter().any(|piece| piece.is_empty()) {
        return Err(DictationError::InvalidShortcut);
    }
    if pieces.len() == 1 {
        return normalize_function_key(pieces[0]);
    }
    let (key, modifiers) = pieces.split_last().expect("len >= 2");
    let (mut ctrl, mut alt, mut shift) = (false, false, false);
    for modifier in modifiers {
        let flag = match () {
            () if modifier.eq_ignore_ascii_case("ctrl") => &mut ctrl,
            () if modifier.eq_ignore_ascii_case("alt") => &mut alt,
            () if modifier.eq_ignore_ascii_case("shift") => &mut shift,
            () => return Err(DictationError::InvalidShortcut),
        };
        if *flag {
            return Err(DictationError::InvalidShortcut);
        }
        *flag = true;
    }
    if !ctrl && !alt {
        // Shift sozinho é fraco demais: Shift+letra é digitação comum.
        return Err(DictationError::InvalidShortcut);
    }
    let key = if key.eq_ignore_ascii_case("space") {
        "Space".to_owned()
    } else {
        let upper = key.to_ascii_uppercase();
        if upper.len() == 1 && upper.as_bytes()[0].is_ascii_alphanumeric() {
            upper
        } else {
            return Err(DictationError::InvalidShortcut);
        }
    };
    let mut canonical = String::new();
    for (enabled, name) in [(ctrl, "Ctrl"), (alt, "Alt"), (shift, "Shift")] {
        if enabled {
            canonical.push_str(name);
            canonical.push('+');
        }
    }
    canonical.push_str(&key);
    Ok(canonical)
}

fn normalize_function_key(value: &str) -> Result<String, DictationError> {
    let digits = value
        .strip_prefix('F')
        .or_else(|| value.strip_prefix('f'))
        .ok_or(DictationError::InvalidShortcut)?;
    let number: u8 = digits
        .parse()
        .map_err(|_| DictationError::InvalidShortcut)?;
    // F1–F5 ficam de fora: ajuda, renomear e atualizar são usos consagrados.
    if !(6..=24).contains(&number) {
        return Err(DictationError::InvalidShortcut);
    }
    Ok(format!("F{number}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const OVERLAY: &str = "Ctrl+Alt+L";

    #[test]
    fn shortcuts_normalize_to_a_canonical_global_safe_form() {
        for (input, expected) in [
            ("ctrl + space", "Ctrl+Space"),
            ("ALT+CTRL+d", "Ctrl+Alt+D"),
            ("shift+ctrl+2", "Ctrl+Shift+2"),
            ("f9", "F9"),
            ("F24", "F24"),
        ] {
            assert_eq!(normalize_shortcut(input).unwrap(), expected, "{input}");
        }
    }

    #[test]
    fn plain_typing_keys_and_malformed_chords_are_rejected() {
        for input in [
            "Space",
            "A",
            "F1",
            "F5",
            "F25",
            "Shift+A",
            "Ctrl+",
            "+Space",
            "Ctrl+Ctrl+Space",
            "Win+Space",
            "Ctrl+Alt+F4",
            "Ctrl+😀",
            "",
        ] {
            assert_eq!(
                normalize_shortcut(input),
                Err(DictationError::InvalidShortcut),
                "{input}"
            );
        }
    }

    #[test]
    fn settings_reject_the_overlay_shortcut_and_bad_durations() {
        let mut settings = DictationSettings::default();
        assert_eq!(settings.validate(OVERLAY), Ok(()));
        settings.shortcut = "ctrl+alt+l".into();
        assert_eq!(
            settings.validate(OVERLAY),
            Err(DictationError::ShortcutConflict)
        );
        settings.shortcut = "Ctrl+Space".into();
        for duration in [0, 4, 301, u16::MAX] {
            settings.max_duration_secs = duration;
            assert_eq!(
                settings.validate(OVERLAY),
                Err(DictationError::InvalidDuration)
            );
        }
    }

    #[test]
    fn only_model_dictionary_language_codes_are_accepted() {
        assert_eq!(DICTATION_LANGUAGES.len(), 83);
        let mut settings = DictationSettings::default();
        for language in [
            None,
            Some("pt-BR"),
            Some("en-US"),
            Some("ja-JP"),
            Some("nn"),
        ] {
            settings.language = language.map(str::to_owned);
            assert_eq!(settings.validate(OVERLAY), Ok(()), "{language:?}");
        }
        // Aliases e caixa divergente ficam de fora: o frontend envia apenas as
        // tags canônicas e um código fora do dicionário filtraria toda a fala.
        for language in ["pt", "pt-br", "es", "auto", "xx-XX", ""] {
            settings.language = Some(language.to_owned());
            assert_eq!(
                settings.validate(OVERLAY),
                Err(DictationError::InvalidLanguage),
                "{language}"
            );
        }
    }

    #[test]
    fn wire_format_round_trips_and_rejects_unknown_fields() {
        let settings = DictationSettings {
            enabled: true,
            shortcut: "F9".into(),
            activation: ActivationMode::Toggle,
            sound_cue: false,
            show_live_text: false,
            limit_duration: false,
            max_duration_secs: 120,
            input_id: Some("mic-1".into()),
            language: Some("pt-BR".into()),
        };
        let wire = serde_json::to_value(&settings).unwrap();
        assert_eq!(wire["activation"], "toggle");
        assert_eq!(wire["maxDurationSecs"], 120);
        assert_eq!(wire["showLiveText"], false);
        assert_eq!(wire["limitDuration"], false);
        assert_eq!(
            serde_json::from_value::<DictationSettings>(wire).unwrap(),
            settings
        );
        // Preferências salvas antes destes campos continuam válidas: os dois
        // assumem o padrão ligado.
        let old_wire = serde_json::json!({
            "enabled": true, "shortcut": "F9", "activation": "hold",
            "soundCue": true, "maxDurationSecs": 60, "inputId": null,
        });
        let upgraded = serde_json::from_value::<DictationSettings>(old_wire).unwrap();
        assert!(upgraded.show_live_text);
        assert!(upgraded.limit_duration);
        assert!(
            serde_json::from_value::<DictationSettings>(serde_json::json!({
                "enabled": true, "shortcut": "F9", "activation": "hold",
                "soundCue": true, "maxDurationSecs": 60, "inputId": null,
                "path": "secret"
            }))
            .is_err()
        );
    }
}
