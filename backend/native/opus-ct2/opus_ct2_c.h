// C ABI for the AnyCaptions CPU translators (OPUS-MT and M2M100):
// CTranslate2 (int8, CPU) plus SentencePiece, so callers hand over plain UTF-8
// text and receive translated UTF-8 text. Mirrors the shape of nemo_speech's
// nmt.h: opaque handle, explicit destroy, status codes.
#pragma once
#include <stdint.h>

#ifdef _WIN32
#define OPUS_CT2_API __declspec(dllexport)
#else
#define OPUS_CT2_API
#endif

#ifdef __cplusplus
extern "C" {
#endif

enum opus_ct2_status {
    OPUS_CT2_OK = 0,
    OPUS_CT2_INVALID_ARGUMENT = 1,
    OPUS_CT2_FAILURE = 2,
};

typedef struct opus_ct2_translator opus_ct2_translator;

// Loads one translation direction: a CTranslate2 model directory plus the
// SentencePiece models for source and target text. intra_threads <= 0 keeps
// the library default.
OPUS_CT2_API int32_t opus_ct2_create(const char* model_dir,
                                     const char* source_spm,
                                     const char* target_spm,
                                     int32_t intra_threads,
                                     opus_ct2_translator** out);

OPUS_CT2_API void opus_ct2_destroy(opus_ct2_translator* translator);

// Translates one sentence. `prefix` is an optional target-language token the
// multi-target OPUS models expect as the first source token (e.g. ">>pob<<");
// NULL or "" means none. On OPUS_CT2_OK, *out receives a NUL-terminated UTF-8
// string owned by the library; release it with opus_ct2_result_destroy.
OPUS_CT2_API int32_t opus_ct2_translate(opus_ct2_translator* translator,
                                        const char* text,
                                        const char* prefix,
                                        char** out);

// Translates one sentence with a many-to-many model (M2M100), where one model
// serves every language pair. `source_token` (e.g. "__en__") leads the source
// text and `target_token` (e.g. "__pt__") starts the decoder; both are
// required and must exist in the model's vocabulary. The target token is not
// part of the returned text. The result is owned and released as above.
OPUS_CT2_API int32_t opus_ct2_translate_to(opus_ct2_translator* translator,
                                           const char* text,
                                           const char* source_token,
                                           const char* target_token,
                                           char** out);

OPUS_CT2_API void opus_ct2_result_destroy(char* result);

// Version of this wrapper, for diagnostics.
OPUS_CT2_API const char* opus_ct2_version(void);

#ifdef __cplusplus
}
#endif
