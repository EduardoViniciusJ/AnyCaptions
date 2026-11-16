#include "opus_ct2_c.h"

#include <cstdlib>
#include <cstring>
#include <memory>
#include <string>
#include <vector>

#include <ctranslate2/translator.h>
#include <sentencepiece_processor.h>

struct opus_ct2_translator {
    std::unique_ptr<ctranslate2::Translator> translator;
    sentencepiece::SentencePieceProcessor source_sp;
    sentencepiece::SentencePieceProcessor target_sp;
};

namespace {

// Longest translation of one caption sentence, in tokens. Captions are short;
// the bound keeps a confused decoder from running to the library default.
constexpr size_t kMaxDecodingLength = 256;

// Greedy decoding: live captions need latency more than the small quality
// edge of the default beam of 2.
ctranslate2::TranslationOptions caption_options() {
    ctranslate2::TranslationOptions options;
    options.beam_size = 1;
    options.max_decoding_length = kMaxDecodingLength;
    return options;
}

// Hands `tokens` back to the caller as detokenized, malloc-owned UTF-8.
int32_t return_text(const sentencepiece::SentencePieceProcessor& target_sp,
                    const std::vector<std::string>& tokens,
                    char** out) {
    std::string decoded;
    if (!target_sp.Decode(tokens, &decoded).ok()) {
        return OPUS_CT2_FAILURE;
    }
    char* result = static_cast<char*>(std::malloc(decoded.size() + 1));
    if (!result) {
        return OPUS_CT2_FAILURE;
    }
    std::memcpy(result, decoded.c_str(), decoded.size() + 1);
    *out = result;
    return OPUS_CT2_OK;
}

}  // namespace

extern "C" {

int32_t opus_ct2_create(const char* model_dir,
                        const char* source_spm,
                        const char* target_spm,
                        int32_t intra_threads,
                        opus_ct2_translator** out) {
    if (!model_dir || !source_spm || !target_spm || !out) {
        return OPUS_CT2_INVALID_ARGUMENT;
    }
    *out = nullptr;
    try {
        auto handle = std::make_unique<opus_ct2_translator>();
        if (!handle->source_sp.Load(source_spm).ok() ||
            !handle->target_sp.Load(target_spm).ok()) {
            return OPUS_CT2_INVALID_ARGUMENT;
        }
        ctranslate2::ReplicaPoolConfig config;
        if (intra_threads > 0) {
            config.num_threads_per_replica = static_cast<size_t>(intra_threads);
        }
        handle->translator = std::make_unique<ctranslate2::Translator>(
            std::string(model_dir),
            ctranslate2::Device::CPU,
            ctranslate2::ComputeType::INT8,
            std::vector<int>{0},
            /*tensor_parallel=*/false,
            config);
        *out = handle.release();
        return OPUS_CT2_OK;
    } catch (...) {
        return OPUS_CT2_FAILURE;
    }
}

void opus_ct2_destroy(opus_ct2_translator* translator) {
    delete translator;
}

int32_t opus_ct2_translate(opus_ct2_translator* translator,
                           const char* text,
                           const char* prefix,
                           char** out) {
    if (!translator || !text || !out) {
        return OPUS_CT2_INVALID_ARGUMENT;
    }
    *out = nullptr;
    try {
        std::vector<std::string> tokens;
        if (!translator->source_sp.Encode(std::string(text), &tokens).ok()) {
            return OPUS_CT2_FAILURE;
        }
        if (prefix && prefix[0] != '\0') {
            tokens.insert(tokens.begin(), std::string(prefix));
        }
        // Marian models expect the end-of-sentence token; without it the
        // decoder degenerates into repetition loops.
        tokens.emplace_back("</s>");
        const auto results = translator->translator->translate_batch(
            {std::move(tokens)}, caption_options());
        if (results.size() != 1 || results[0].hypotheses.empty()) {
            return OPUS_CT2_FAILURE;
        }
        return return_text(translator->target_sp, results[0].output(), out);
    } catch (...) {
        return OPUS_CT2_FAILURE;
    }
}

int32_t opus_ct2_translate_to(opus_ct2_translator* translator,
                              const char* text,
                              const char* source_token,
                              const char* target_token,
                              char** out) {
    if (!translator || !text || !out || !source_token || !target_token ||
        source_token[0] == 0 || target_token[0] == 0) {
        return OPUS_CT2_INVALID_ARGUMENT;
    }
    *out = nullptr;
    try {
        std::vector<std::string> tokens;
        if (!translator->source_sp.Encode(std::string(text), &tokens).ok()) {
            return OPUS_CT2_FAILURE;
        }
        // M2M100 input: source language token, the sentence, end of sentence.
        tokens.insert(tokens.begin(), std::string(source_token));
        tokens.emplace_back("</s>");
        // The decoder is told the language by its first token.
        const std::vector<std::vector<std::string>> target_prefix = {
            {std::string(target_token)}};
        const auto results = translator->translator->translate_batch(
            {std::move(tokens)}, target_prefix, caption_options());
        if (results.size() != 1 || results[0].hypotheses.empty()) {
            return OPUS_CT2_FAILURE;
        }
        std::vector<std::string> output = results[0].output();
        if (!output.empty() && output.front() == target_token) {
            output.erase(output.begin());
        }
        return return_text(translator->target_sp, output, out);
    } catch (...) {
        return OPUS_CT2_FAILURE;
    }
}

void opus_ct2_result_destroy(char* result) {
    std::free(result);
}

const char* opus_ct2_version(void) {
    return "0.2.0";
}

}  // extern "C"
