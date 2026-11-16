# Builds opus_ct2_c.dll: CTranslate2 (CPU, int8 via Ruy) and SentencePiece
# linked into a small C-ABI wrapper the app loads at runtime. The two
# libraries are cloned into OPUS_CT2_DEPS_DIR first; see README.md.
cmake_minimum_required(VERSION 3.18)
project(opus_ct2_c CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)

set(OPUS_CT2_DEPS_DIR "${CMAKE_CURRENT_SOURCE_DIR}/deps" CACHE PATH
    "Folder holding the CTranslate2 and sentencepiece checkouts")

# CTranslate2: CPU only; Ruy provides the int8 kernels without MKL/oneDNN.
set(WITH_MKL OFF CACHE BOOL "" FORCE)
set(WITH_DNNL OFF CACHE BOOL "" FORCE)
set(WITH_RUY ON CACHE BOOL "" FORCE)
set(WITH_CUDA OFF CACHE BOOL "" FORCE)
set(WITH_CUDNN OFF CACHE BOOL "" FORCE)
set(OPENMP_RUNTIME "NONE" CACHE STRING "" FORCE)
set(BUILD_CLI OFF CACHE BOOL "" FORCE)
set(BUILD_TESTS OFF CACHE BOOL "" FORCE)
add_subdirectory("${OPUS_CT2_DEPS_DIR}/CTranslate2" CTranslate2)

# SentencePiece: static library, no tools.
set(SPM_ENABLE_SHARED OFF CACHE BOOL "" FORCE)
set(SPM_ENABLE_TCMALLOC OFF CACHE BOOL "" FORCE)
add_subdirectory("${OPUS_CT2_DEPS_DIR}/sentencepiece" sentencepiece)

add_library(opus_ct2_c SHARED opus_ct2_c.cpp opus_ct2_c.h)
target_include_directories(opus_ct2_c PRIVATE
    "${OPUS_CT2_DEPS_DIR}/CTranslate2/include"
    "${OPUS_CT2_DEPS_DIR}/sentencepiece/src"
    # sentencepiece_processor.h includes Abseil, fetched by sentencepiece.
    ${CMAKE_CURRENT_BINARY_DIR}/_deps/abseil-cpp-src)
target_link_libraries(opus_ct2_c PRIVATE ctranslate2 sentencepiece-static)
