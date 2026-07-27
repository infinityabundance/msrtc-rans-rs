// Derived from Microsoft MLVC (MIT).
// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.
//
// oracle_cli.cpp — deterministic casefile harness for msrtc_rans
//
// Reads a binary casefile from stdin (or argv[1]), encodes the
// given indices/values with the specified PMF, writes the raw
// bitstream to stdout, and emits JSON metadata to stderr.
//
// Binary casefile format (little-endian):
//   uint32_t variant;          // 0=Rans64, 1=RansByte
//   uint32_t symbol_bits;
//   uint32_t bypass_bits;
//   uint32_t pmf_lengths_count;
//   int32_t  pmf_lengths[pmf_lengths_count];
//   uint32_t pmf_offsets_count;
//   int32_t  pmf_offsets[pmf_offsets_count];
//   uint32_t pmf_table_count;
//   int32_t  pmf_table[pmf_table_count];
//   uint32_t indices_count;
//   int32_t  indices[indices_count];
//   uint32_t values_count;
//   int32_t  values[values_count];
//
// Stderr output (last line):
//   {"status":"ok","hex":"...","sha256":"...","length":N}
//   or
//   {"status":"error","message":"..."}

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>

#include <openssl/sha.h>

#include <msrtc_rans/EntropyCoder.h>

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

static std::string to_hex(const msrtc_rans::span<const std::byte>& data) {
    static const char hex[] = "0123456789abcdef";
    std::string out;
    out.reserve(data.size() * 2);
    for (size_t i = 0; i < data.size(); ++i) {
        auto b = static_cast<uint8_t>(data[i]);
        out.push_back(hex[b >> 4]);
        out.push_back(hex[b & 0xf]);
    }
    return out;
}

static std::string sha256_hex(const msrtc_rans::span<const std::byte>& data) {
    uint8_t hash[SHA256_DIGEST_LENGTH];
    SHA256(reinterpret_cast<const uint8_t*>(data.data()), data.size(), hash);

    static const char hex[] = "0123456789abcdef";
    std::string out;
    out.reserve(SHA256_DIGEST_LENGTH * 2);
    for (size_t i = 0; i < SHA256_DIGEST_LENGTH; ++i) {
        out.push_back(hex[hash[i] >> 4]);
        out.push_back(hex[hash[i] & 0xf]);
    }
    return out;
}

// Read the entire contents of a FILE* into a byte vector.
static std::vector<uint8_t> read_all(FILE* f) {
    std::vector<uint8_t> buf;
    uint8_t chunk[4096];
    size_t n;
    while ((n = fread(chunk, 1, sizeof(chunk), f)) > 0) {
        buf.insert(buf.end(), chunk, chunk + n);
    }
    return buf;
}

// ---------------------------------------------------------------------------
// Binary reader  (little-endian x86-64 — no byteswap needed)
// ---------------------------------------------------------------------------

struct CaseFile {
    uint32_t            variant;
    uint32_t            symbol_bits;
    uint32_t            bypass_bits;
    std::vector<int32_t> pmf_lengths;
    std::vector<int32_t> pmf_offsets;
    std::vector<int32_t> pmf_table;
    std::vector<int32_t> indices;
    std::vector<int32_t> values;
};

static bool read_u32(const uint8_t*& ptr, const uint8_t* end, uint32_t& out) {
    if (ptr + 4 > end) return false;
    out = static_cast<uint32_t>(ptr[0])
        | (static_cast<uint32_t>(ptr[1]) << 8)
        | (static_cast<uint32_t>(ptr[2]) << 16)
        | (static_cast<uint32_t>(ptr[3]) << 24);
    ptr += 4;
    return true;
}

static bool read_i32(const uint8_t*& ptr, const uint8_t* end, int32_t& out) {
    uint32_t tmp;
    if (!read_u32(ptr, end, tmp)) return false;
    out = static_cast<int32_t>(tmp);
    return true;
}

static bool read_i32_array(const uint8_t*& ptr, const uint8_t* end,
                           std::vector<int32_t>& out) {
    uint32_t count;
    if (!read_u32(ptr, end, count)) return false;
    out.resize(count);
    for (uint32_t i = 0; i < count; ++i) {
        if (!read_i32(ptr, end, out[i])) return false;
    }
    return true;
}

static bool parse_casefile(const std::vector<uint8_t>& raw, CaseFile& cf) {
    const uint8_t* ptr = raw.data();
    const uint8_t* end = ptr + raw.size();

    if (!read_u32(ptr, end, cf.variant))    return false;
    if (!read_u32(ptr, end, cf.symbol_bits)) return false;
    if (!read_u32(ptr, end, cf.bypass_bits)) return false;

    if (!read_i32_array(ptr, end, cf.pmf_lengths)) return false;
    if (!read_i32_array(ptr, end, cf.pmf_offsets)) return false;
    if (!read_i32_array(ptr, end, cf.pmf_table))   return false;
    if (!read_i32_array(ptr, end, cf.indices))     return false;
    if (!read_i32_array(ptr, end, cf.values))      return false;

    if (cf.indices.size() != cf.values.size()) return false;
    return true;
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

int main(int argc, char* argv[]) {
    // ---- read casefile ----------------------------------------------------
    std::vector<uint8_t> raw;
    if (argc > 1) {
        FILE* f = std::fopen(argv[1], "rb");
        if (!f) {
            std::fprintf(stderr, R"({"status":"error","message":"cannot open %s"})" "\n", argv[1]);
            return 1;
        }
        raw = read_all(f);
        std::fclose(f);
    } else {
        // Read from stdin (binary)
        if (std::freopen(nullptr, "rb", stdin)) {
            // stdin was reopened binary — good
        }
        raw = read_all(stdin);
    }

    CaseFile cf;
    if (!parse_casefile(raw, cf)) {
        std::fprintf(stderr, R"({"status":"error","message":"invalid casefile"})" "\n");
        return 1;
    }

    // ---- map variant ------------------------------------------------------
    msrtc_rans::RansVariant variant;
    if (cf.variant == 0) {
        variant = msrtc_rans::RansVariant::Rans64;
    } else if (cf.variant == 1) {
        variant = msrtc_rans::RansVariant::RansByte;
    } else {
        std::fprintf(stderr, R"({"status":"error","message":"unknown variant %u"})" "\n", cf.variant);
        return 1;
    }

    // ---- initialize encoder ------------------------------------------------
    msrtc_rans::EntropyEncoder encoder;
    auto ec = encoder.Initialize(
        variant,
        cf.pmf_lengths,
        cf.pmf_offsets,
        cf.pmf_table,
        static_cast<int>(cf.symbol_bits),
        static_cast<int>(cf.bypass_bits));
    if (ec) {
        std::fprintf(stderr, R"({"status":"error","message":"init: %s"})" "\n", ec.message().c_str());
        return 1;
    }

    // ---- encode -----------------------------------------------------------
    msrtc_rans::HeapResizableBuffer buffer;
    auto encoded = encoder.Encode(buffer, cf.indices, cf.values);
    if (encoded.is_empty()) {
        std::fprintf(stderr, R"({"status":"error","message":"encode returned empty"})" "\n");
        return 1;
    }

    // ---- write raw bitstream to stdout (binary) ---------------------------
    if (std::freopen(nullptr, "wb", stdout)) {
        // stdout was reopened binary — good
    }
    auto raw_bytes = reinterpret_cast<const uint8_t*>(encoded.data());
    size_t written = std::fwrite(raw_bytes, 1, encoded.size(), stdout);
    if (written != encoded.size()) {
        std::fprintf(stderr, R"({"status":"error","message":"short write to stdout"})" "\n");
        return 1;
    }
    std::fflush(stdout);

    // ---- emit JSON metadata to stderr -------------------------------------
    auto hex  = to_hex(encoded);
    auto sha  = sha256_hex(encoded);
    std::fprintf(stderr, R"({"status":"ok","hex":"%s","sha256":"%s","length":%zu})" "\n",
                 hex.c_str(), sha.c_str(), encoded.size());

    return 0;
}
