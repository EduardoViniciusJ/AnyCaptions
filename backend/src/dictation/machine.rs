//! Máquina de estados do ditado: ocioso → gravando → finalizando.
//!
//! A máquina só decide; quem abre o microfone, transcreve e injeta o texto é
//! o chamador, guiado pelas [`DictationAction`]s devolvidas. O relógio entra
//! por parâmetro (`Instant`) para que os testes controlem o tempo.

use std::time::{Duration, Instant};

use super::ActivationMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DictationPhase {
    Idle,
    Recording,
    Finalizing,
}

/// Por que uma gravação terminou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinishReason {
    KeyReleased,
    Toggled,
    TimedOut,
}

/// O que o chamador deve fazer após um evento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DictationAction {
    /// Abrir o microfone e começar a reconhecer.
    Start { session: u64 },
    /// Fechar o microfone, concluir o reconhecimento e injetar o texto.
    Finish { session: u64, reason: FinishReason },
    /// Fechar o microfone e descartar tudo, sem injetar.
    Discard { session: u64 },
}

pub struct DictationMachine {
    activation: ActivationMode,
    /// `None` = sem teto: a gravação só termina por ação do usuário ou erro.
    max_duration: Option<Duration>,
    phase: DictationPhase,
    /// Estado físico da tecla: o auto-repeat do Windows reenvia `Pressed`
    /// enquanto a tecla está presa, e esses repetidos devem ser ignorados.
    key_down: bool,
    session: u64,
    recording_since: Option<Instant>,
}

impl DictationMachine {
    pub fn new(activation: ActivationMode, max_duration: Option<Duration>) -> Self {
        Self {
            activation,
            max_duration,
            phase: DictationPhase::Idle,
            key_down: false,
            session: 0,
            recording_since: None,
        }
    }

    pub fn phase(&self) -> DictationPhase {
        self.phase
    }

    pub fn session(&self) -> u64 {
        self.session
    }

    pub fn key_pressed(&mut self, now: Instant) -> Option<DictationAction> {
        if self.key_down {
            return None; // auto-repeat da tecla presa
        }
        self.key_down = true;
        match (self.phase, self.activation) {
            (DictationPhase::Idle, _) => Some(self.start(now)),
            (DictationPhase::Recording, ActivationMode::Toggle) => {
                Some(self.finish(FinishReason::Toggled))
            }
            _ => None,
        }
    }

    pub fn key_released(&mut self) -> Option<DictationAction> {
        self.key_down = false;
        match (self.phase, self.activation) {
            (DictationPhase::Recording, ActivationMode::Hold) => {
                Some(self.finish(FinishReason::KeyReleased))
            }
            _ => None,
        }
    }

    /// Esc: descarta a gravação em andamento sem injetar nada.
    pub fn cancel(&mut self) -> Option<DictationAction> {
        if self.phase != DictationPhase::Recording {
            return None;
        }
        self.phase = DictationPhase::Idle;
        self.recording_since = None;
        Some(DictationAction::Discard {
            session: self.session,
        })
    }

    /// Deve ser chamada periodicamente durante a gravação: aplica o teto de
    /// duração nos dois modos (alternar esquecido ou tecla presa no segurar).
    /// Sem teto configurado, nunca corta.
    pub fn tick(&mut self, now: Instant) -> Option<DictationAction> {
        let max_duration = self.max_duration?;
        let elapsed = now.duration_since(self.recording_since?);
        if self.phase == DictationPhase::Recording && elapsed >= max_duration {
            return Some(self.finish(FinishReason::TimedOut));
        }
        None
    }

    /// O reconhecedor terminou (texto injetado ou descartado). Só a sessão
    /// viva devolve a máquina ao ocioso; conclusões atrasadas são ignoradas.
    pub fn finished(&mut self, session: u64) -> bool {
        if self.phase == DictationPhase::Finalizing && session == self.session {
            self.phase = DictationPhase::Idle;
            true
        } else {
            false
        }
    }

    fn start(&mut self, now: Instant) -> DictationAction {
        self.session = self.session.wrapping_add(1);
        self.phase = DictationPhase::Recording;
        self.recording_since = Some(now);
        DictationAction::Start {
            session: self.session,
        }
    }

    fn finish(&mut self, reason: FinishReason) -> DictationAction {
        self.phase = DictationPhase::Finalizing;
        self.recording_since = None;
        DictationAction::Finish {
            session: self.session,
            reason,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hold() -> DictationMachine {
        DictationMachine::new(ActivationMode::Hold, Some(Duration::from_secs(60)))
    }

    fn toggle() -> DictationMachine {
        DictationMachine::new(ActivationMode::Toggle, Some(Duration::from_secs(60)))
    }

    #[test]
    fn hold_records_while_pressed_and_finishes_on_release() {
        let mut machine = hold();
        let now = Instant::now();
        assert_eq!(
            machine.key_pressed(now),
            Some(DictationAction::Start { session: 1 })
        );
        // Auto-repeat do Windows com a tecla presa não gera novas ações.
        assert_eq!(machine.key_pressed(now), None);
        assert_eq!(machine.key_pressed(now), None);
        assert_eq!(machine.phase(), DictationPhase::Recording);
        assert_eq!(
            machine.key_released(),
            Some(DictationAction::Finish {
                session: 1,
                reason: FinishReason::KeyReleased,
            })
        );
        assert_eq!(machine.phase(), DictationPhase::Finalizing);
        assert!(machine.finished(1));
        assert_eq!(machine.phase(), DictationPhase::Idle);
    }

    #[test]
    fn toggle_starts_and_finishes_on_separate_taps() {
        let mut machine = toggle();
        let now = Instant::now();
        assert_eq!(
            machine.key_pressed(now),
            Some(DictationAction::Start { session: 1 })
        );
        // Soltar não finaliza no modo alternar.
        assert_eq!(machine.key_released(), None);
        assert_eq!(machine.phase(), DictationPhase::Recording);
        // Segurar a tecla no modo alternar não pode iniciar e finalizar sozinho.
        assert_eq!(
            machine.key_pressed(now),
            Some(DictationAction::Finish {
                session: 1,
                reason: FinishReason::Toggled,
            })
        );
        assert_eq!(machine.key_pressed(now), None);
        assert_eq!(machine.key_released(), None);
    }

    #[test]
    fn cancel_discards_only_a_live_recording() {
        let mut machine = hold();
        assert_eq!(machine.cancel(), None);
        machine.key_pressed(Instant::now());
        assert_eq!(
            machine.cancel(),
            Some(DictationAction::Discard { session: 1 })
        );
        assert_eq!(machine.phase(), DictationPhase::Idle);
        // A tecla continua presa; soltá-la depois do cancelamento é inofensivo.
        assert_eq!(machine.key_released(), None);
        // E o próximo pressionar começa uma sessão nova.
        assert_eq!(
            machine.key_pressed(Instant::now()),
            Some(DictationAction::Start { session: 2 })
        );
    }

    #[test]
    fn recording_times_out_in_both_modes() {
        for mut machine in [hold(), toggle()] {
            let start = Instant::now();
            machine.key_pressed(start);
            assert_eq!(machine.tick(start + Duration::from_secs(59)), None);
            assert_eq!(
                machine.tick(start + Duration::from_secs(60)),
                Some(DictationAction::Finish {
                    session: 1,
                    reason: FinishReason::TimedOut,
                })
            );
            // Depois de finalizar, o tick fica quieto.
            assert_eq!(machine.tick(start + Duration::from_secs(61)), None);
        }
    }

    #[test]
    fn without_a_limit_the_recording_never_times_out() {
        let mut machine = DictationMachine::new(ActivationMode::Toggle, None);
        let start = Instant::now();
        machine.key_pressed(start);
        assert_eq!(machine.tick(start + Duration::from_secs(60 * 60)), None);
        assert_eq!(machine.phase(), DictationPhase::Recording);
    }

    #[test]
    fn stale_completions_cannot_revive_or_close_a_newer_session() {
        let mut machine = toggle();
        let now = Instant::now();
        machine.key_pressed(now);
        machine.key_released();
        machine.key_pressed(now); // finaliza a sessão 1
        machine.key_released();
        assert!(!machine.finished(99)); // sessão desconhecida é ignorada
        assert!(machine.finished(1));
        machine.key_pressed(now); // sessão 2 gravando
        assert!(!machine.finished(1)); // conclusão atrasada não mexe na sessão 2
        assert_eq!(machine.phase(), DictationPhase::Recording);
    }

    #[test]
    fn idle_release_and_finalizing_press_are_ignored() {
        let mut machine = hold();
        assert_eq!(machine.key_released(), None);
        let now = Instant::now();
        machine.key_pressed(now);
        machine.key_released();
        assert_eq!(machine.phase(), DictationPhase::Finalizing);
        // Pressionar durante a finalização não inicia outra gravação.
        assert_eq!(machine.key_pressed(now), None);
        machine.key_released();
        assert!(machine.finished(1));
    }
}
