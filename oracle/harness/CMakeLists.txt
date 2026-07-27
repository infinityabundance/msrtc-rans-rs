# Derived from Microsoft MLVC (MIT).
# Copyright (c) Microsoft Corporation.
# Licensed under the MIT license.
#
# CMakeLists.txt — standalone oracle CLI harness for msrtc_rans
#
# This project builds the msrtc_rans static library from the upstream
# MLVC sources (included via relative path) and links it into the
# oracle_cli executable.

cmake_minimum_required(VERSION 3.23)
project(msrtc_oracle_cli LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

# Path to the upstream MLVC msrtc_rans source (relative to this file)
set(MSRTC_RANS_SRC_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../mlvc-upstream/packages/msrtc_rans")

# Find OpenSSL for SHA-256
find_package(OpenSSL REQUIRED)

# ---- msrtc_rans static library (from upstream) ----------------------------
set(MSRTC_RANS_SRCS
    "${MSRTC_RANS_SRC_DIR}/src/EntropyCoder.cpp"
)

add_library(msrtc_rans STATIC ${MSRTC_RANS_SRCS})

target_include_directories(msrtc_rans
    PUBLIC  "${MSRTC_RANS_SRC_DIR}/include"
    PRIVATE "${MSRTC_RANS_SRC_DIR}/private/include"
)

target_compile_definitions(msrtc_rans PRIVATE msrtc_rans_EXPORTS)

# ---- oracle_cli executable -------------------------------------------------
add_executable(oracle_cli oracle_cli.cpp)

target_include_directories(oracle_cli PRIVATE
    "${MSRTC_RANS_SRC_DIR}/include"
    "${MSRTC_RANS_SRC_DIR}/private/include"
)

target_link_libraries(oracle_cli PRIVATE
    msrtc_rans
    OpenSSL::Crypto
)
