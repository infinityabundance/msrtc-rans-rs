// Derived from Microsoft MLVC (MIT).
// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.
//
// stream_oracle_cli.cpp — deterministic multipart stream harness for msrtc_rans
//
// Exercises Microsoft's persistent `RansEncoderStream` / `RansDecoderStream`:
//
//   stream_oracle_cli encode [casefile]
//       Reads a multipart casefile, pushes every batch into a single
//       RansEncoderStream (persistent raw rANS state), flushes once, and
//       writes the stream bytes to stdout. JSON metadata goes to stderr.
//
//   stream_oracle_cli decode <streamfile> [casefile]
//       Reads the multipart casefile plus an encoded stream file, opens a
//       RansDecoderStream, decodes the batches in reverse push order
//       (last pushed = decoded first, matching the LIFO stream layout),
//       verifies EOF, and emits the decoded values as JSON on stderr.
//
// Binary multipart casefile format (little-endian):
//   uint32_t variant;             // 0=Rans64, 1=RansByte
//   uint32_t symbol_bits;
//   uint32_t bypass_bits;
//   uint32_t batch_count;
//   per batch (in push order):
//     uint32_t pmf_lengths_count; int32_t pmf_lengths[...];
//     uint32_t pmf_offsets_count; int32_t pmf_offsets[...];
//     uint32_t pmf_table_count;   int32_t pmf_table[...];
//     uint32_t indices_count;     int32_t indices[...];
//     uint32_t values_count;      int32_t values[...];
//
// Stderr output (encode, last line):
//   {"status":"ok","hex":"...","sha256":"...","length":N}
// Stderr output (decode, last line):
//   {"status":"ok","batches":N,"values":[[...],[...]],"values_sha256":"...","eof":true}

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

static std::string sha256_hex(const void* ptr, size_t size) {
    uint8_t hash[SHA256_DIGEST_LENGTH];
    SHA256(static_cast<const uint8_t*>(ptr), size, hash);

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

static std::vector<uint8_t> read_file(const char* path) {
    FILE* f = std::fopen(path, "rb");
    if (!f) {
        return {};
    }
    auto raw = read_all(f);
    std::fclose(f);
    return raw;
}

// ---------------------------------------------------------------------------
// Binary reader (little-endian x86-64 — no byteswap needed)
// ---------------------------------------------------------------------------

struct StreamBatch {
    std::vector<int32_t> pmf_lengths;
    std::vector<int32_t> pmf_offsets;
    std::vector<int32_t> pmf_table;
    std::vector<int32_t> indices;
    std::vector<int32_t> values;
};

struct StreamCaseFile {
    uint32_t             variant;
    uint32_t             symbol_bits;
    uint32_t             bypass_bits;
    std::vector<StreamBatch> batches;
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

static bool parse_casefile(const std::vector<uint8_t>& raw, StreamCaseFile& cf) {
    const uint8_t* ptr = raw.data();
    const uint8_t* end = ptr + raw.size();

    if (!read_u32(ptr, end, cf.variant))     return false;
    if (!read_u32(ptr, end, cf.symbol_bits)) return false;
    if (!read_u32(ptr, end, cf.bypass_bits)) return false;

    uint32_t batch_count;
    if (!read_u32(ptr, end, batch_count)) return false;
    if (batch_count == 0) return false;

    cf.batches.resize(batch_count);
    for (uint32_t b = 0; b < batch_count; ++b) {
        auto& batch = cf.batches[b];
        if (!read_i32_array(ptr, end, batch.pmf_lengths)) return false;
        if (!read_i32_array(ptr, end, batch.pmf_offsets)) return false;
        if (!read_i32_array(ptr, end, batch.pmf_table))   return false;
        if (!read_i32_array(ptr, end, batch.indices))     return false;
        if (!read_i32_array(ptr, end, batch.values))      return false;
        if (batch.indices.size() != batch.values.size())  return false;
    }
    return true;
}

// Parse a casefile and report how many bytes it consumed (so a trailing
// payload — the encoded stream — can be split off).
static bool parse_casefile_split(const std::vector<uint8_t>& raw, StreamCaseFile& cf,
                                 size_t& consumed) {
    const uint8_t* start = raw.data();
    const uint8_t* ptr = start;
    const uint8_t* end = ptr + raw.size();

    if (!read_u32(ptr, end, cf.variant))     return false;
    if (!read_u32(ptr, end, cf.symbol_bits)) return false;
    if (!read_u32(ptr, end, cf.bypass_bits)) return false;

    uint32_t batch_count;
    if (!read_u32(ptr, end, batch_count)) return false;
    if (batch_count == 0) return false;

    cf.batches.resize(batch_count);
    for (uint32_t b = 0; b < batch_count; ++b) {
        auto& batch = cf.batches[b];
        if (!read_i32_array(ptr, end, batch.pmf_lengths)) return false;
        if (!read_i32_array(ptr, end, batch.pmf_offsets)) return false;
        if (!read_i32_array(ptr, end, batch.pmf_table))   return false;
        if (!read_i32_array(ptr, end, batch.indices))     return false;
        if (!read_i32_array(ptr, end, batch.values))      return false;
        if (batch.indices.size() != batch.values.size())  return false;
    }
    consumed = static_cast<size_t>(ptr - start);
    return true;
}

static bool map_variant(uint32_t v, msrtc_rans::RansVariant& out) {
    switch (v) {
    case 0: out = msrtc_rans::RansVariant::Rans64; return true;
    case 1: out = msrtc_rans::RansVariant::RansByte; return true;
    default: return false;
    }
}

// ---------------------------------------------------------------------------
// Encode mode
// ---------------------------------------------------------------------------

static int run_encode(const StreamCaseFile& cf) {
    msrtc_rans::RansVariant variant;
    if (!map_variant(cf.variant, variant)) {
        std::fprintf(stderr, R"({"status":"error","message":"unknown variant %u"})" "\n", cf.variant);
        return 1;
    }

    // Reusable resizable buffer + persistent encoder stream
    msrtc_rans::HeapResizableBuffer buffer;
    msrtc_rans::RansEncoderStream stream;
    auto ec = stream.Initialize(variant, buffer);
    if (ec) {
        std::fprintf(stderr, R"({"status":"error","message":"stream init: %s"})" "\n", ec.message().c_str());
        return 1;
    }

    for (const auto& batch : cf.batches) {
        msrtc_rans::EntropyEncoder encoder;
        auto e = encoder.Initialize(
            variant,
            batch.pmf_lengths,
            batch.pmf_offsets,
            batch.pmf_table,
            static_cast<int>(cf.symbol_bits),
            static_cast<int>(cf.bypass_bits));
        if (e) {
            std::fprintf(stderr, R"({"status":"error","message":"encoder init: %s"})" "\n", e.message().c_str());
            return 1;
        }
        e = encoder.Encode(stream, batch.indices, batch.values);
        if (e) {
            std::fprintf(stderr, R"({"status":"error","message":"encode: %s"})" "\n", e.message().c_str());
            return 1;
        }
    }

    auto data = stream.Flush();
    if (data.is_empty()) {
        std::fprintf(stderr, R"({"status":"error","message":"flush returned empty"})" "\n");
        return 1;
    }

    if (std::freopen(nullptr, "wb", stdout)) {
        // stdout was reopened binary — good
    }
    auto raw_bytes = reinterpret_cast<const uint8_t*>(data.data());
    size_t written = std::fwrite(raw_bytes, 1, data.size(), stdout);
    if (written != data.size()) {
        std::fprintf(stderr, R"({"status":"error","message":"short write to stdout"})" "\n");
        return 1;
    }
    std::fflush(stdout);

    auto hex = to_hex(data);
    auto sha = sha256_hex(data.data(), data.size());
    std::fprintf(stderr, R"({"status":"ok","hex":"%s","sha256":"%s","length":%zu})" "\n",
                 hex.c_str(), sha.c_str(), data.size());
    return 0;
}

// ---------------------------------------------------------------------------
// Decode mode
// ---------------------------------------------------------------------------

static int run_decode(const StreamCaseFile& cf, const std::vector<uint8_t>& streamBytes) {
    msrtc_rans::RansVariant variant;
    if (!map_variant(cf.variant, variant)) {
        std::fprintf(stderr, R"({"status":"error","message":"unknown variant %u"})" "\n", cf.variant);
        return 1;
    }

    msrtc_rans::RansDecoderStream stream;
    auto ec = stream.Initialize(variant);
    if (ec) {
        std::fprintf(stderr, R"({"status":"error","message":"stream init: %s"})" "\n", ec.message().c_str());
        return 1;
    }
    auto span = msrtc_rans::span<const std::byte>{
        reinterpret_cast<const std::byte*>(streamBytes.data()), streamBytes.size()};
    ec = stream.Open(span);
    if (ec) {
        std::fprintf(stderr, R"({"status":"error","message":"stream open: %s"})" "\n", ec.message().c_str());
        return 1;
    }

    // Decode batches in REVERSE push order (last pushed = decoded first)
    std::string valuesJson = "[";
    std::vector<uint8_t> valuesBytes;
    const size_t batchCount = cf.batches.size();
    for (size_t i = batchCount; i-- > 0;) {
        const auto& batch = cf.batches[i];

        msrtc_rans::EntropyDecoder decoder;
        auto e = decoder.Initialize(
            variant,
            batch.pmf_lengths,
            batch.pmf_offsets,
            batch.pmf_table,
            static_cast<int>(cf.symbol_bits),
            static_cast<int>(cf.bypass_bits));
        if (e) {
            std::fprintf(stderr, R"({"status":"error","message":"decoder init: %s"})" "\n", e.message().c_str());
            return 1;
        }

        std::vector<int32_t> decoded(batch.indices.size(), 0);
        e = decoder.Decode(decoded, batch.indices, stream);
        if (e) {
            std::fprintf(stderr, R"({"status":"error","message":"decode: %s"})" "\n", e.message().c_str());
            return 1;
        }

        if (i != batchCount - 1) valuesJson += ",";
        valuesJson += "[";
        for (size_t v = 0; v < decoded.size(); ++v) {
            if (v != 0) valuesJson += ",";
            valuesJson += std::to_string(decoded[v]);
        }
        valuesJson += "]";

        for (int32_t v : decoded) {
            auto b = reinterpret_cast<const uint8_t*>(&v);
            valuesBytes.insert(valuesBytes.end(), b, b + sizeof(v));
        }
    }
    valuesJson += "]";

    if (!stream.CheckEOF()) {
        std::fprintf(stderr, R"({"status":"error","message":"stream not at EOF after decode"})" "\n");
        return 1;
    }

    auto valuesSha = sha256_hex(valuesBytes.data(), valuesBytes.size());
    std::fprintf(stderr, R"({"status":"ok","batches":%zu,"values":%s,"values_sha256":"%s","eof":true})" "\n",
                 batchCount, valuesJson.c_str(), valuesSha.c_str());
    return 0;
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

int main(int argc, char* argv[]) {
    if (argc < 2) {
        std::fprintf(stderr, R"({"status":"error","message":"usage: stream_oracle_cli encode [casefile] | decode <streamfile> [casefile]"})" "\n");
        return 1;
    }

    std::string mode = argv[1];

    if (mode == "encode") {
        // Casefile: argv[2] if present, else stdin
        std::vector<uint8_t> raw;
        if (argc > 2) {
            raw = read_file(argv[2]);
            if (raw.empty()) {
                std::fprintf(stderr, R"({"status":"error","message":"cannot open %s"})" "\n", argv[2]);
                return 1;
            }
        } else {
            if (std::freopen(nullptr, "rb", stdin)) {
                // stdin was reopened binary — good
            }
            raw = read_all(stdin);
        }

        StreamCaseFile cf;
        if (!parse_casefile(raw, cf)) {
            std::fprintf(stderr, R"({"status":"error","message":"invalid casefile"})" "\n");
            return 1;
        }
        return run_encode(cf);
    }

    if (mode == "decode") {
        // Read ALL of stdin: [casefile bytes][stream bytes]
        if (std::freopen(nullptr, "rb", stdin)) {
            // stdin was reopened binary — good
        }
        auto all = read_all(stdin);

        StreamCaseFile cf;
        size_t consumed = 0;
        if (!parse_casefile_split(all, cf, consumed)) {
            std::fprintf(stderr, R"({"status":"error","message":"invalid casefile"})" "\n");
            return 1;
        }
        std::vector<uint8_t> streamBytes(all.begin() + static_cast<ptrdiff_t>(consumed), all.end());
        if (streamBytes.empty()) {
            std::fprintf(stderr, R"({"status":"error","message":"missing stream data"})" "\n");
            return 1;
        }
        return run_decode(cf, streamBytes);
    }

    std::fprintf(stderr, R"({"status":"error","message":"unknown mode %s"})" "\n", mode.c_str());
    return 1;
}
