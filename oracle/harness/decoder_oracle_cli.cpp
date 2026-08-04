// Derived from Microsoft MLVC (MIT).
// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.
//
// decoder_oracle_cli.cpp — raw rANS decoder oracle
//
// Exercises raw RansDecoder templates: Init, Get, Advance, CheckEOF.
//
// Binary input format (little-endian):
//   uint32_t variant;         // 0=Rans64, 1=RansByte
//   uint32_t scale_bits;      // shared scale_bits for decode
//   uint32_t op_count;        // number of decode operations
//   uint32_t encoded_len;     // length of encoded bitstream in bytes
//   // encoded_len bytes of raw bitstream
//   // op_count × (start:uint32_t, freq:uint32_t)
//
// Stdout: the full sequence of Get() return values as little-endian uint32_t,
//         plus final EOF status as uint32_t (1=EOF, 0=not EOF).
//
// Stderr output (last line):
//   {"status":"ok","get_values":[N1,N2,...],"eof":true,"op_count":N}
//   or
//   {"status":"error","message":"..."}

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <exception>
#include <string>
#include <vector>

#include <msrtc_rans/rans.h>
#include <msrtc_rans/span.h>

// ---------------------------------------------------------------------------
// SpanSource — adapter that lets msrtc_rans::span serve as a RansDecoder Source
// ---------------------------------------------------------------------------

template <typename UnitType>
struct SpanSource {
    using unit_t = UnitType;

    msrtc_rans::span<const UnitType> m_data;
    size_t m_pos;

    SpanSource(msrtc_rans::span<const UnitType> data)
        : m_data(data), m_pos(0) {}

    bool operator()(UnitType& unit) {
        if (m_pos >= m_data.size()) {
            return false;
        }
        unit = m_data[m_pos];
        ++m_pos;
        return true;
    }

    bool OnOK() { return true; }
    bool OnInvalidStream() { return false; }
    bool IsOpen() const { return true; }
    bool IsEOF() const { return m_pos >= m_data.size(); }
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

// Binary reader helpers (little-endian)
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
// Decode helpers (templated on StateType / UnitType)
// ---------------------------------------------------------------------------

template <typename StateType, typename UnitType>
static bool decode_stream(
    uint32_t scale_bits,
    const std::vector<UnitType>& units,
    const std::vector<std::pair<uint32_t, uint32_t>>& symbols,
    std::vector<uint32_t>& get_values,
    bool& eof)
{
    get_values.clear();
    get_values.reserve(symbols.size());

    // Create a source over the units
    SpanSource<UnitType> src_span(
        msrtc_rans::span<const UnitType>(units.data(), units.size())
    );

    msrtc_rans::RansDecoder<StateType, UnitType, SpanSource<UnitType>> decoder(std::move(src_span));

    // Init
    if (!decoder.Init()) {
        return false;
    }

    // Decode each symbol — rANS decodes in REVERSE of the encode order.
    for (auto it = symbols.rbegin(); it != symbols.rend(); ++it) {
        uint32_t start = it->first;
        uint32_t freq  = it->second;

        // Get cumulative frequency
        uint32_t cum_freq = decoder.Get(static_cast<msrtc_rans::rans_freq_t>(scale_bits));
        get_values.push_back(cum_freq);

        // Advance past the symbol
        if (!decoder.Advance(start, freq, static_cast<msrtc_rans::rans_freq_t>(scale_bits))) {
            return false;
        }
    }

    // Check EOF
    eof = decoder.CheckEOF();
    return true;
}

// Convert a byte vector to a vector of uint32_t (little-endian)
static std::vector<uint32_t> bytes_to_u32s(const std::vector<uint8_t>& bytes)
{
    size_t count = bytes.size() / sizeof(uint32_t);
    std::vector<uint32_t> result(count);
    for (size_t i = 0; i < count; ++i) {
        result[i] = static_cast<uint32_t>(bytes[i * 4])
                  | (static_cast<uint32_t>(bytes[i * 4 + 1]) << 8)
                  | (static_cast<uint32_t>(bytes[i * 4 + 2]) << 16)
                  | (static_cast<uint32_t>(bytes[i * 4 + 3]) << 24);
    }
    return result;
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
    uint32_t scale_bits;
    uint32_t op_count;
    uint32_t encoded_len;

    if (!read_u32(ptr, end, variant)) {
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
    if (!read_u32(ptr, end, encoded_len)) {
        std::fprintf(stderr, R"({"status":"error","message":"truncated header"})" "\n");
        return 1;
    }

    if (variant > 1) {
        std::fprintf(stderr, R"({"status":"error","message":"unknown variant %u"})" "\n", variant);
        return 1;
    }

    // ---- read bitstream bytes ----------------------------------------------
    if (ptr + encoded_len > end) {
        std::fprintf(stderr, R"({"status":"error","message":"truncated bitstream"})" "\n");
        return 1;
    }
    std::vector<uint8_t> bitstream_bytes(ptr, ptr + encoded_len);
    ptr += encoded_len;

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

    // ---- decode ------------------------------------------------------------
    std::vector<uint32_t> get_values;
    bool eof = false;
    bool decode_ok = false;

    try {
        if (variant == 0) { // Rans64
            auto units = bytes_to_u32s(bitstream_bytes);
            decode_ok = decode_stream<uint64_t, uint32_t>(scale_bits, units, symbols, get_values, eof);
        } else { // RansByte
            decode_ok = decode_stream<uint32_t, uint8_t>(scale_bits, bitstream_bytes, symbols, get_values, eof);
        }
    } catch (std::exception& ex) {
        std::fprintf(stderr, R"({"status":"error","message":"decode exception: %s"})" "\n", ex.what());
        return 1;
    }

    if (!decode_ok) {
        std::fprintf(stderr, "{\"status\":\"error\",\"message\":\"decode failed (init/advance error)\"}\n");
        return 1;
    }

    // ---- write Get() values + EOF status to stdout (binary) ----------------
    if (std::freopen(nullptr, "wb", stdout)) {
    }

    for (auto val : get_values) {
        uint8_t buf[4];
        buf[0] = static_cast<uint8_t>(val);
        buf[1] = static_cast<uint8_t>(val >> 8);
        buf[2] = static_cast<uint8_t>(val >> 16);
        buf[3] = static_cast<uint8_t>(val >> 24);
        if (std::fwrite(buf, 1, 4, stdout) != 4) {
            std::fprintf(stderr, R"({"status":"error","message":"short write to stdout"})" "\n");
            return 1;
        }
    }

    // Write EOF status as uint32_t (1=EOF, 0=not EOF)
    {
        uint32_t eof_val = eof ? 1u : 0u;
        uint8_t buf[4];
        buf[0] = static_cast<uint8_t>(eof_val);
        buf[1] = static_cast<uint8_t>(eof_val >> 8);
        buf[2] = static_cast<uint8_t>(eof_val >> 16);
        buf[3] = static_cast<uint8_t>(eof_val >> 24);
        if (std::fwrite(buf, 1, 4, stdout) != 4) {
            std::fprintf(stderr, R"({"status":"error","message":"short write to stdout"})" "\n");
            return 1;
        }
    }
    std::fflush(stdout);

    // ---- emit JSON metadata to stderr --------------------------------------
    std::fprintf(stderr, R"({"status":"ok","get_values":[)");

    for (size_t i = 0; i < get_values.size(); ++i) {
        if (i > 0) {
            std::fprintf(stderr, ",");
        }
        std::fprintf(stderr, "%u", get_values[i]);
    }

    std::fprintf(stderr, R"(],"eof":%s,"op_count":%u})" "\n",
                 eof ? "true" : "false", op_count);

    return 0;
}
