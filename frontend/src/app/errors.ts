import { t } from "../i18n";

export function errorMessage(error: unknown): string {
  const code =
    typeof error === "object" && error !== null && "code" in error
      ? error.code
      : null;
  const kind =
    typeof error === "object" && error !== null && "kind" in error
      ? error.kind
      : null;
  if (kind === "dictation") {
    switch (code) {
      case "invalid-shortcut":
        return t("errDictationShortcutFormat");
      case "shortcut-conflict":
        return t("errDictationShortcutConflict");
      case "invalid-duration":
        return t("errDictationDuration");
      case "invalid-language":
        return t("errDictationLanguage");
      default:
        return t("errDictation");
    }
  }
  switch (code) {
    case "capture-unavailable":
      return t("errCapture");
    case "transcription-unavailable":
      return t("errTranscription");
    case "translation-unavailable":
      return t("errTranslation");
    case "invalid-font-size":
      return t("errFontSize");
    case "recovery-unavailable":
      return t("errRecovery");
    case "invalid-shortcut":
      return t("errShortcutFormat");
    case "shortcut-unavailable":
      return t("errShortcutTaken");
    case "not-found":
      return t("errOutputGone");
    case "invalid-id":
      return t("errOutputInvalid");
    default:
      break;
  }
  return t("errGeneric");
}
