use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CaptionMode {
    #[default]
    EnEn,
    EnPt,
}

impl CaptionMode {
    pub fn requires_translation(self) -> bool {
        match self {
            Self::EnEn => false,
            Self::EnPt => true,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CaptureState {
    #[default]
    Stopped,
    Preparing,
    Listening,
    Processing,
    Error,
}

impl CaptureState {
    fn permits(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Stopped, Self::Preparing)
                | (
                    Self::Preparing,
                    Self::Listening | Self::Error | Self::Stopped
                )
                | (
                    Self::Listening,
                    Self::Processing | Self::Error | Self::Stopped
                )
                | (
                    Self::Processing,
                    Self::Listening | Self::Error | Self::Stopped
                )
                | (Self::Error, Self::Stopped)
        )
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindowMode {
    #[default]
    Edit,
    Locked,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CaptionColor {
    #[default]
    White,
    Yellow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Appearance {
    pub font_size: u16,
    pub color: CaptionColor,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            font_size: 32,
            color: CaptionColor::White,
        }
    }
}

impl Appearance {
    fn validate(self) -> Result<(), DomainError> {
        if !(20..=56).contains(&self.font_size) {
            return Err(DomainError::InvalidFontSize);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Resources {
    pub audio_capture: bool,
    pub transcription: bool,
    pub translation: bool,
}

impl Resources {
    fn validate(self, mode: CaptionMode) -> Result<(), DomainError> {
        if !self.audio_capture {
            return Err(DomainError::CaptureUnavailable);
        }
        if !self.transcription {
            return Err(DomainError::TranscriptionUnavailable);
        }
        if mode.requires_translation() && !self.translation {
            return Err(DomainError::TranslationUnavailable);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DomainError {
    InvalidTransition,
    InvalidFontSize,
    CaptureUnavailable,
    TranscriptionUnavailable,
    TranslationUnavailable,
    RecoveryUnavailable,
    CounterExhausted,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub revision: u32,
    pub session_id: u32,
    pub mode: CaptionMode,
    pub capture: CaptureState,
    pub window_mode: WindowMode,
    pub appearance: Appearance,
    pub resources: Resources,
    pub can_start: bool,
    pub can_lock: bool,
    pub error: Option<DomainError>,
}

#[derive(Debug, Default, Clone)]
pub struct AppState {
    revision: u32,
    session_id: u32,
    mode: CaptionMode,
    capture: CaptureState,
    appearance: Appearance,
    resources: Resources,
    error: Option<DomainError>,
}

impl AppState {
    pub fn snapshot(&self) -> AppSnapshot {
        AppSnapshot {
            revision: self.revision,
            session_id: self.session_id,
            mode: self.mode,
            capture: self.capture,
            window_mode: WindowMode::Edit,
            appearance: self.appearance,
            resources: self.resources,
            can_start: self.capture == CaptureState::Stopped
                && self.resources.validate(self.mode).is_ok(),
            can_lock: false,
            error: self.error,
        }
    }

    fn advance(&mut self, new_session: bool) -> Result<(), DomainError> {
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(DomainError::CounterExhausted)?;
        let session_id = self
            .session_id
            .checked_add(u32::from(new_session))
            .ok_or(DomainError::CounterExhausted)?;
        self.revision = revision;
        self.session_id = session_id;
        Ok(())
    }

    pub fn set_mode(&mut self, mode: CaptionMode) -> Result<(), DomainError> {
        if mode != self.mode {
            self.advance(true)?;
            self.mode = mode;
            self.capture = CaptureState::Stopped;
            self.error = None;
        }
        Ok(())
    }

    pub fn start(&mut self) -> Result<(), DomainError> {
        self.resources.validate(self.mode)?;
        if !self.capture.permits(CaptureState::Preparing) {
            return Err(DomainError::InvalidTransition);
        }
        self.advance(true)?;
        self.capture = CaptureState::Preparing;
        self.error = None;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), DomainError> {
        if self.capture != CaptureState::Stopped {
            self.advance(true)?;
            self.capture = CaptureState::Stopped;
            self.error = None;
        }
        Ok(())
    }

    // Future workers must identify their session; late work cannot revive a stopped capture.
    pub fn transition(&mut self, session_id: u32, next: CaptureState) -> Result<(), DomainError> {
        if session_id != self.session_id
            || !self.capture.permits(next)
            || matches!(
                next,
                CaptureState::Preparing | CaptureState::Stopped | CaptureState::Error
            )
        {
            return Err(DomainError::InvalidTransition);
        }
        self.advance(false)?;
        self.capture = next;
        Ok(())
    }

    pub fn fail(&mut self, session_id: u32, error: DomainError) -> Result<(), DomainError> {
        if session_id != self.session_id || !self.capture.permits(CaptureState::Error) {
            return Err(DomainError::InvalidTransition);
        }
        self.advance(false)?;
        self.capture = CaptureState::Error;
        self.error = Some(error);
        Ok(())
    }

    pub fn set_appearance(&mut self, appearance: Appearance) -> Result<(), DomainError> {
        appearance.validate()?;
        if appearance != self.appearance {
            self.advance(false)?;
            self.appearance = appearance;
        }
        Ok(())
    }

    pub fn set_window_mode(&mut self, mode: WindowMode) -> Result<(), DomainError> {
        match mode {
            WindowMode::Edit => Ok(()),
            // Recovery via tray/shortcut must exist before any native click-through is enabled.
            WindowMode::Locked => Err(DomainError::RecoveryUnavailable),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready() -> AppState {
        AppState {
            resources: Resources {
                audio_capture: true,
                transcription: true,
                translation: true,
            },
            ..AppState::default()
        }
    }

    #[test]
    fn starts_stopped_in_edit_mode_without_available_resources() {
        let state = AppState::default().snapshot();
        assert_eq!(state.mode, CaptionMode::EnEn);
        assert_eq!(state.capture, CaptureState::Stopped);
        assert_eq!(state.window_mode, WindowMode::Edit);
        assert!(!state.can_start && !state.can_lock);
        assert_eq!(state.appearance.font_size, 32);
    }

    #[test]
    fn portuguese_output_requires_an_additional_translator() {
        let mut state = ready();
        state.resources.translation = false;
        assert!(!CaptionMode::EnEn.requires_translation());
        assert!(CaptionMode::EnPt.requires_translation());
        assert!(state.start().is_ok());
        state.set_mode(CaptionMode::EnPt).unwrap();
        assert_eq!(state.start(), Err(DomainError::TranslationUnavailable));
        assert_eq!(state.snapshot().capture, CaptureState::Stopped);
    }

    #[test]
    fn missing_capture_or_transcription_prevents_start() {
        let mut state = AppState::default();
        assert_eq!(state.start(), Err(DomainError::CaptureUnavailable));
        state.resources.audio_capture = true;
        assert_eq!(state.start(), Err(DomainError::TranscriptionUnavailable));
        assert_eq!(state.snapshot().revision, 0);
    }

    #[test]
    fn valid_lifecycle_rejects_duplicates_and_stale_results() {
        let mut state = ready();
        assert_eq!(
            state.transition(0, CaptureState::Listening),
            Err(DomainError::InvalidTransition)
        );
        state.start().unwrap();
        let session = state.snapshot().session_id;
        assert_eq!(state.start(), Err(DomainError::InvalidTransition));
        state.transition(session, CaptureState::Listening).unwrap();
        state.transition(session, CaptureState::Processing).unwrap();
        state.transition(session, CaptureState::Listening).unwrap();
        state.stop().unwrap();
        let revision = state.snapshot().revision;
        state.stop().unwrap();
        assert_eq!(state.snapshot().revision, revision);
        state.start().unwrap();
        assert_eq!(
            state.transition(session, CaptureState::Listening),
            Err(DomainError::InvalidTransition)
        );
    }

    #[test]
    fn changing_mode_invalidates_work_and_clears_errors() {
        let mut state = ready();
        state.start().unwrap();
        let session = state.snapshot().session_id;
        state
            .fail(session, DomainError::TranscriptionUnavailable)
            .unwrap();
        assert_eq!(state.snapshot().capture, CaptureState::Error);
        state.set_mode(CaptionMode::EnPt).unwrap();
        assert!(state.snapshot().error.is_none());
        assert_eq!(state.snapshot().capture, CaptureState::Stopped);
        assert_ne!(state.snapshot().session_id, session);
        assert_eq!(
            state.fail(session, DomainError::TranslationUnavailable),
            Err(DomainError::InvalidTransition)
        );
    }

    #[test]
    fn appearance_is_validated_before_mutation() {
        let mut state = AppState::default();
        for size in [0, 19, 57, u16::MAX] {
            assert_eq!(
                state.set_appearance(Appearance {
                    font_size: size,
                    color: CaptionColor::Yellow
                }),
                Err(DomainError::InvalidFontSize)
            );
        }
        assert_eq!(state.snapshot().appearance, Appearance::default());
        for size in [20, 56] {
            state
                .set_appearance(Appearance {
                    font_size: size,
                    color: CaptionColor::Yellow,
                })
                .unwrap();
            assert_eq!(state.snapshot().appearance.font_size, size);
        }
    }

    #[test]
    fn locking_without_recovery_is_rejected() {
        let mut state = AppState::default();
        assert_eq!(
            state.set_window_mode(WindowMode::Locked),
            Err(DomainError::RecoveryUnavailable)
        );
        assert_eq!(state.snapshot().window_mode, WindowMode::Edit);
    }

    #[test]
    fn unsupported_modes_and_unexpected_fields_are_rejected() {
        for value in ["pt-pt", "pt-en", "auto", "EN-EN"] {
            assert!(serde_json::from_value::<CaptionMode>(serde_json::json!(value)).is_err());
        }
        assert!(
            serde_json::from_value::<Appearance>(
                serde_json::json!({"fontSize":32,"color":"white","path":"secret"})
            )
            .is_err()
        );
        assert!(
            serde_json::from_value::<Appearance>(
                serde_json::json!({"fontSize":-1,"color":"white"})
            )
            .is_err()
        );
        assert!(
            serde_json::from_value::<Appearance>(serde_json::json!({"fontSize":32,"color":"red"}))
                .is_err()
        );
    }
}
