// Derived from Microsoft MLVC (MIT).
// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.
//
// raw_oracle_cli.cpp — raw rANS primitive oracle
//
// Exercises RawRansEncoder::Put, RawRansEncoder::Put(symbol), and Flush
// at the primitive level, bypassing the high-level EntropyEncoder.
//
// Binary input format (little-endian):
//   uint32_t variant;      // 0=Rans64, 1=RansByte
//   uint32_t mode;         // 0=raw_put(start,freq,scale_bits), 1=prepared_put(symbol)
//   uint32_t scale_bits;   // shared scale_bits for all symbols
//   uint32_t op_count;     // number of encode operations
//   // op_count × (start:uint32_t, freq:uint32_t)
//
// Stderr output (last line):
//   {"status":"ok","hex":"...","sha256":"...","length":N,"mode":0}
//   or
//   {"status":"error","message":"..."}
//
// Stdout: raw bitstream bytes

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <exception>
#include <string>
#include <utility>
#include <vector>

#include <openssl/sha.h>

#include <msrtc_rans/rans.h>

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

static std::string to_hex(const std::vector<uint8_t>& data)
{
    static const char hex[] = "0123456789abcdef";
    std::string out;
    out.reserve(data.size() * 2);
    for (auto b : data) {
        out.push_back(hex[b >> 4]);
        out.push_back(hex[b & 0xf]);
    }
    return out;
}

static std::string sha256_hex(const std::vector<uint8_t>& data)
{
    uint8_t hash[SHA256_DIGEST_LENGTH];
    SHA256(data.data(), data.size(), hash);

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
static std::vector<uint8_t> read_all(FILE* f)
{
    std::vector<uint8_t> buf;
    uint8_t chunk[4096];
    size_t n;
    while ((n = fread(chunk, 1, sizeof(chunk), f)) > 0) {
        buf.insert(buf.end(), chunk, chunk + n);
    }
    return buf;
}

// ---------------------------------------------------------------------------
// Sink: collects units from the rANS encoder (which writes from end to
// start).  At the end, get_bytes() reverses the collected units so the
// output byte stream is in the correct order.
// ---------------------------------------------------------------------------
template <typename UnitType>
struct CollectSink {
    std::vector<UnitType> data;

    void operator()(UnitType v) { data.push_back(v); }

    // Turn the collected units into a byte stream in the correct order.
    // rANS writes the last unit first, so we walk the vector backwards.
    std::vector<uint8_t> get_bytes() const
    {
        std::vector<uint8_t> bytes;
        bytes.reserve(data.size() * sizeof(UnitType));
        for (size_t i = data.size(); i > 0; --i) {
            UnitType v = data[i - 1];
            for (size_t b = 0; b < sizeof(UnitType); ++b) {
                bytes.push_back(static_cast<uint8_t>(v >> (b * 8)));
            }
        }
        return bytes;
    }
};

// ---------------------------------------------------------------------------
// Binary reader helpers (little-endian)
// ---------------------------------------------------------------------------
static bool read_u32(const uint8_t*& ptr, const uint8_t* end, uint32_t& out)
{
    if (ptr + 4 > end)
        return false;
    out = static_cast<uint32_t>(ptr[0]) | (static_cast<uint32_t>(ptr[1]) << 8)
        | (static_cast<uint32_t>(ptr[2]) << 16) | (static_cast<uint32_t>(ptr[3]) << 24);
    ptr += 4;
    return true;
}

// ---------------------------------------------------------------------------
// Encoding helpers (templated on StateType / UnitType)
// ---------------------------------------------------------------------------

// Mode 0: raw Put(start, freq, scale_bits)
template <typename StateType, typename UnitType>
static std::vector<uint8_t> encode_raw(
    uint32_t scale_bits, const std::vector<std::pair<uint32_t, uint32_t>>& symbols)
{
    CollectSink<UnitType> sink;
    msrtc_rans::RansEncoder<StateType, UnitType, CollectSink<UnitType>> encoder(std::move(sink));
    for (auto& sym : symbols) {
        encoder.Put(sym.first, sym.second, static_cast<msrtc_rans::rans_freq_t>(scale_bits));
    }
    encoder.Flush();
    return encoder.GetSink().get_bytes();
}

// Mode 1: prepared Put(symbol) with precomputed RansEncSymbol
template <typename StateType, typename UnitType>
static std::vector<uint8_t> encode_prepared(
    uint32_t scale_bits, const std::vector<std::pair<uint32_t, uint32_t>>& symbols)
{
    // Precompute RansEncSymbol for each (start, freq) tuple
    std::vector<msrtc_rans::RansEncSymbol<StateType, UnitType>> prepared;
    prepared.reserve(symbols.size());
    for (auto& sym : symbols) {
        prepared.emplace_back(sym.first, sym.second, static_cast<msrtc_rans::rans_freq_t>(scale_bits));
    }

    CollectSink<UnitType> sink;
    msrtc_rans::RansEncoder<StateType, UnitType, CollectSink<UnitType>> encoder(std::move(sink));
    for (auto& psym : prepared) {
        encoder.Put(psym);
    }
    encoder.Flush();
    return encoder.GetSink().get_bytes();
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

int main(int argc, char* argv[])
{
    // ---- read input --------------------------------------------------------
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
        if (std::freopen(nullptr, "rb", stdin)) {
        }
        raw = read_all(stdin);
    }

    // ---- parse header ------------------------------------------------------
    const uint8_t* ptr = raw.data();
    const uint8_t* end = ptr + raw.size();

    uint32_t variant;
    uint32_t mode;
    uint32_t scale_bits;
    uint32_t op_count;

    if (!read_u32(ptr, end, variant)) {
        std::fprintf(stderr, R"({"status":"error","message":"truncated header"})" "\n");
        return 1;
    }
    if (!read_u32(ptr, end, mode)) {
        std::fprintf(stderr, R"({"status":"error","message":"truncated header"})" "\n");
        return 1;
    }
    if (!read_u32(ptr, end, scale_bits)) {
        std::fprintf(stderr, R"({"status":"error","message":"truncated header"})" "\n");
        return 1;
    }
    if (!read_u32(ptr, end, op_count)) {
        std::fprintf(stderr, R"({"status":"error","message":"truncated header"})" "\n");
        return 1;
    }

    if (variant > 1) {
        std::fprintf(stderr, R"({"status":"error","message":"unknown variant %u"})" "\n", variant);
        return 1;
    }
    if (mode > 1) {
        std::fprintf(stderr, R"({"status":"error","message":"unknown mode %u"})" "\n", mode);
        return 1;
    }

    // ---- parse symbol tuples -----------------------------------------------
    std::vector<std::pair<uint32_t, uint32_t>> symbols;
    symbols.reserve(op_count);
    for (uint32_t i = 0; i < op_count; ++i) {
        uint32_t start, freq;
        if (!read_u32(ptr, end, start) || !read_u32(ptr, end, freq)) {
            std::fprintf(stderr, R"({"status":"error","message":"truncated symbol data"})" "\n");
            return 1;
        }
        symbols.emplace_back(start, freq);
    }

    // ---- encode ------------------------------------------------------------
    std::vector<uint8_t> output;

    try {
        if (variant == 0) { // Rans64
            if (mode == 0) {
                output = encode_raw<uint64_t, uint32_t>(scale_bits, symbols);
            } else {
                output = encode_prepared<uint64_t, uint32_t>(scale_bits, symbols);
            }
        } else { // RansByte
            if (mode == 0) {
                output = encode_raw<uint32_t, uint8_t>(scale_bits, symbols);
            } else {
                output = encode_prepared<uint32_t, uint8_t>(scale_bits, symbols);
            }
        }
    } catch (std::exception& ex) {
        std::fprintf(stderr, R"({"status":"error","message":"encode exception: %s"})" "\n", ex.what());
        return 1;
    }

    // ---- write bitstream to stdout (binary) --------------------------------
    if (std::freopen(nullptr, "wb", stdout)) {
    }
    size_t written = std::fwrite(output.data(), 1, output.size(), stdout);
    if (written != output.size()) {
        std::fprintf(stderr, R"({"status":"error","message":"short write to stdout"})" "\n");
        return 1;
    }
    std::fflush(stdout);

    // ---- emit JSON metadata to stderr --------------------------------------
    auto hex = to_hex(output);
    auto sha = sha256_hex(output);
    std::fprintf(stderr, R"({"status":"ok","hex":"%s","sha256":"%s","length":%zu,"mode":%u})" "\n",
                 hex.c_str(), sha.c_str(), output.size(), mode);

    return 0;
}
