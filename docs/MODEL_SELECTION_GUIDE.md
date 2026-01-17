# Model Selection Guide

This guide helps you choose the right transcription engine and model for your use case. The choice depends on your hardware, language needs, performance requirements, and whether you're on battery power.

> **Important: Non-European Language Speakers**
>
> If you speak **Japanese, Chinese, Korean, Arabic, Hindi, Bengali, Vietnamese, Indonesian, Thai, Persian, Hebrew, Swahili**, or any other non-European language, **use Whisper** (not Parakeet). With a GPU, use `large-v3-turbo`. On CPU only, use `small` or `base`. Parakeet only supports 25 European languages. See the [summary](#non-european-languages) for configuration examples.

## Quick Decision Matrix

| Situation | Recommended Configuration |
|-----------|--------------------------|
| **English-only, have NVIDIA GPU** | Parakeet TDT 0.6B v3 with CUDA/TensorRT |
| **English-only, AMD GPU** | Parakeet TDT with ROCm or Whisper with Vulkan |
| **English-only, CPU only** | Parakeet TDT (CPU) or Whisper small.en |
| **Multilingual (European languages)** | Parakeet TDT 0.6B v3 |
| **Non-European languages, with GPU** | **Whisper large-v3-turbo** |
| **Non-European languages, CPU only** | **Whisper small or base** |
| **Battery-conscious laptop** | Whisper small.en with on-demand loading |
| **Fastest transcription possible** | Parakeet TDT with TensorRT (NVIDIA) |
| **Low VRAM (< 2GB)** | Whisper tiny.en or base.en |

---

## Part 1: Choosing an Engine

Voxtype supports two transcription engines:

1. **Whisper** (default) - OpenAI's general-purpose speech recognition via whisper.cpp
2. **Parakeet** (optional) - NVIDIA's FastConformer-based model via ONNX Runtime

### Language Support: The Critical Difference

**This is the most important factor in choosing an engine.**

Parakeet only supports **25 European languages**. If you speak any of these languages not supported by Parakeet, you **must use Whisper**:

- **East Asian:** Japanese, Chinese (Mandarin/Cantonese), Korean, Vietnamese, Thai
- **South Asian:** Hindi, Bengali, Tamil, Urdu, Punjabi, Marathi
- **Middle Eastern:** Arabic, Hebrew, Persian (Farsi), Turkish*
- **African:** Swahili, Yoruba, Amharic, Hausa
- **Southeast Asian:** Indonesian, Malay, Tagalog, Burmese

*Turkish is supported in Parakeet v3

**If you speak any of these languages, skip to the Whisper section and use `large-v3-turbo`.**

### Whisper: Universal Language Support

**Strengths:**
- Broad language support (99+ languages including all those listed above)
- Well-tested, mature codebase
- Works on any hardware (CPU, NVIDIA, AMD, Intel)
- Single binary, no external dependencies
- Flexible model sizes from 39M to 1.5B parameters

**Weaknesses:**
- Slower than Parakeet for equivalent accuracy
- Encoder-decoder architecture has higher latency
- GPU support requires specific build flags

**Best for:** Multilingual transcription, non-European languages, maximum hardware compatibility

### Parakeet: Speed + Accuracy for European Languages

**Strengths:**
- Dramatically faster inference (up to 3000x real-time factor with TensorRT)
- State-of-the-art accuracy (~6% WER, #1 on HuggingFace ASR leaderboard)
- Built-in punctuation and capitalization
- Accurate word-level timestamps
- Frame-skipping TDT decoder reduces compute
- Works on CPU, AMD GPU (ROCm), and NVIDIA GPU (CUDA/TensorRT)

**Weaknesses:**
- Limited to 25 European languages (no Asian, Middle Eastern, or African languages)
- Requires ONNX Runtime (larger binary)
- Single model size (600M parameters)

**Best for:** English and European language transcription, situations where speed matters, systems where you want automatic punctuation

**Important:** Parakeet does NOT require an NVIDIA GPU. It runs on:
- **CPU** - Works out of the box, still faster than many alternatives
- **AMD GPU** - Via ROCm execution provider
- **NVIDIA GPU** - Via CUDA or TensorRT (fastest option)

The NVIDIA-optimized path is the fastest, but Parakeet's efficient architecture makes it competitive even on CPU.

### Decision Flowchart

```
Do you need Japanese, Chinese, Korean, Arabic, Hindi,
or other non-European languages?
    │
    ├─ YES → Use Whisper large-v3-turbo
    │
    └─ NO → Do you only speak English or European languages?
                │
                ├─ YES → Use Parakeet TDT (fastest, best accuracy)
                │
                └─ SOMETIMES → Consider Whisper for flexibility
```

---

## Part 2: Whisper Model Selection

### When You Must Use Whisper

**Use Whisper if you need any of these languages:**

| Region | Languages (examples) |
|--------|---------------------|
| East Asia | Japanese, Chinese, Korean, Vietnamese |
| South Asia | Hindi, Bengali, Tamil, Urdu, Marathi |
| Middle East | Arabic, Hebrew, Persian |
| Africa | Swahili, Yoruba, Amharic |
| Southeast Asia | Indonesian, Tagalog, Thai |

For these languages, the recommended model is **large-v3-turbo**:

```toml
[whisper]
model = "large-v3-turbo"
language = "auto"  # or specify: "ja", "zh", "ko", "ar", "hi", etc.
```

### Available Models

| Model | File to Download | Parameters | VRAM | Speed | Languages |
|-------|------------------|-----------|------|-------|-----------|
| tiny | ggml-tiny.bin | 39M | ~1 GB | ~10x | **99+ (multilingual)** |
| tiny.en | ggml-tiny.en.bin | 39M | ~1 GB | ~10x | English only |
| base | ggml-base.bin | 74M | ~1 GB | ~7x | **99+ (multilingual)** |
| base.en | ggml-base.en.bin | 74M | ~1 GB | ~7x | English only |
| small | ggml-small.bin | 244M | ~2 GB | ~4x | **99+ (multilingual)** |
| small.en | ggml-small.en.bin | 244M | ~2 GB | ~4x | English only |
| medium | ggml-medium.bin | 769M | ~5 GB | ~2x | **99+ (multilingual)** |
| medium.en | ggml-medium.en.bin | 769M | ~5 GB | ~2x | English only |
| large-v3 | ggml-large-v3.bin | 1550M | ~10 GB | 1x | **99+ (multilingual)** |
| large-v3-turbo | ggml-large-v3-turbo.bin | 809M | ~6 GB | ~8x | **99+ (multilingual)** |

**Speed is relative to large-v3 (1x baseline). Higher is faster.*

**Note:** The `.en` models are English-only and faster/more accurate for English. The models without `.en` support 99+ languages including Japanese, Chinese, Korean, Arabic, Hindi, etc. There are no `.en` variants for large-v3 or large-v3-turbo.

### .en Models vs Multilingual Models

Whisper models come in two variants. **This distinction is critical for non-European language support.**

| Model | File | Languages |
|-------|------|-----------|
| `small.en` | ggml-small.en.bin | English only |
| `small` | ggml-small.bin | 99+ languages (including Japanese, Chinese, Arabic, Hindi, etc.) |

**English-only models (`.en` suffix):**
- Faster (no language detection overhead)
- More accurate for English
- **Cannot transcribe non-European languages**
- Use when you only speak English

**Multilingual models (no `.en` suffix):**
- Support 99+ languages including all Asian, Middle Eastern, and African languages
- Slightly slower due to language detection
- **Required for Japanese, Chinese, Korean, Arabic, Hindi, Vietnamese, etc.**
- Use when you speak non-European languages or switch between languages

**Important:** If you need Japanese, Chinese, Arabic, Hindi, or other non-European languages, you must use the multilingual model (e.g., `small` not `small.en`). The `.en` models will not work for these languages.

### Model Recommendations by Use Case

#### Best for Non-European Languages

For Japanese, Chinese, Korean, Arabic, Hindi, and dozens of other languages, you must use Whisper.

**With GPU (recommended):**
```toml
[whisper]
model = "large-v3-turbo"
language = "auto"  # Auto-detect, or set specific language code
```

The turbo model is:
- 8x faster than large-v3
- Within 1-2% accuracy of the full model
- Requires ~6GB VRAM

**CPU only:**
```toml
[whisper]
model = "small"   # or "base" for lower-end hardware
language = "auto"
```

On CPU, large-v3-turbo is very slow. Use `small` for a good balance of speed and accuracy, or `base` if speed is critical.

**Note:** Both multilingual (`small`, `base`) and English-only (`small.en`, `base.en`) models are available in `voxtype setup model`. For non-European languages, make sure to select the version **without** the `.en` suffix.

#### Best for CPU (English): small.en

```toml
[whisper]
model = "small.en"
language = "en"
```

The small.en model offers the best accuracy-to-speed ratio for CPU-only systems:
- Transcribes ~4x faster than large-v3
- Fits comfortably in 2GB VRAM or system RAM
- Noticeably more accurate than base/tiny

#### Best for GPU (English): large-v3-turbo

```toml
[whisper]
model = "large-v3-turbo"
language = "en"
```

Even for English, large-v3-turbo is excellent if you have a GPU.

#### Best for Laptops: small.en with on-demand loading

```toml
[whisper]
model = "small.en"
language = "en"
on_demand_loading = true
gpu_isolation = true
```

For laptop users concerned about battery and heat:
- **on_demand_loading:** Model loads only when you start recording, unloads after transcription
- **gpu_isolation:** Runs transcription in subprocess that exits completely, releasing GPU memory

#### Best for Low-End Hardware: tiny.en or base.en

```toml
[whisper]
model = "base.en"
language = "en"
```

For older systems, Raspberry Pi, or integrated graphics:
- base.en: ~142MB, runs on nearly anything
- tiny.en: ~75MB, for extreme constraints

---

## Part 3: Parakeet Model Selection

### Language Limitations

**Parakeet v3 supports only these 25 languages:**

English, German, French, Spanish, Italian, Dutch, Polish, Portuguese, Romanian, Czech, Hungarian, Slovak, Slovenian, Danish, Norwegian, Swedish, Finnish, Greek, Turkish, Ukrainian, Russian, Catalan, Galician, Basque

**If your language is not in this list, use Whisper large-v3-turbo instead.**

### Available Models

| Model | Architecture | Punctuation | Speed | Accuracy | Recommended |
|-------|-------------|-------------|-------|----------|-------------|
| parakeet-tdt-0.6b-v3 | TDT | Yes | Fast | ~6% WER | **Yes** |
| parakeet-ctc-0.6b | CTC | No | Faster | ~7% WER | Special cases |

### TDT vs CTC

**TDT (Token-Duration-Transducer)** - Recommended:
- Proper punctuation and capitalization
- Accurate word-level timestamps
- Frame-skipping for efficiency
- Better handling of pauses and phrasing

**CTC (Connectionist Temporal Classification)**:
- Character-level output (no punctuation)
- Slightly faster inference
- Use only if you have a post-processing pipeline that handles punctuation

### Configuration

```toml
engine = "parakeet"

[parakeet]
model = "parakeet-tdt-0.6b-v3"
# model_type auto-detected, or set explicitly:
# model_type = "tdt"
```

### GPU Acceleration Options

Parakeet runs on multiple hardware configurations via ONNX Runtime:

| Feature Flag | Backend | Hardware | Performance |
|--------------|---------|----------|-------------|
| `parakeet` | CPU | Any | Good |
| `parakeet-rocm` | ROCm | AMD GPUs | Very good |
| `parakeet-cuda` | CUDA | NVIDIA GPUs | Excellent |
| `parakeet-tensorrt` | TensorRT | NVIDIA GPUs | Best |

Build with the appropriate feature:

```bash
# CPU only (works on any system)
cargo build --release --features parakeet

# AMD GPU with ROCm
cargo build --release --features parakeet-rocm

# NVIDIA GPU with CUDA
cargo build --release --features parakeet-cuda

# NVIDIA GPU with TensorRT (fastest)
cargo build --release --features parakeet-tensorrt
```

### Parakeet Without a GPU

Parakeet's efficient FastConformer architecture means it performs well even on CPU:

```toml
engine = "parakeet"

[parakeet]
model = "parakeet-tdt-0.6b-v3"
```

Build for CPU: `cargo build --release --features parakeet`

On a modern CPU, Parakeet TDT typically outperforms Whisper models of similar accuracy because of the efficient frame-skipping TDT decoder. You still get automatic punctuation and capitalization.

---

## Part 4: Hardware Considerations

### VRAM Requirements Summary

| Model | Minimum VRAM | Recommended |
|-------|-------------|-------------|
| Whisper tiny/base | 1 GB | 2 GB |
| Whisper small | 2 GB | 4 GB |
| Whisper medium | 5 GB | 6 GB |
| Whisper large-v3-turbo | 6 GB | 8 GB |
| Whisper large-v3 | 10 GB | 12 GB |
| Parakeet TDT 0.6B | N/A (CPU) | 4+ GB (GPU) |

### CPU Performance Expectations

Real-world transcription times for 10 seconds of audio on a modern CPU (AMD Ryzen 7 / Intel i7):

**Whisper:**
| Model | Approx Time |
|-------|-------------|
| tiny | 1-2s |
| base | 2-3s |
| small | 4-6s |
| medium | 10-15s |
| large-v3 | 25-40s |

**Parakeet:**
| Model | Approx Time |
|-------|-------------|
| TDT 0.6B (CPU) | 2-4s |
| TDT 0.6B (CUDA) | <1s |
| TDT 0.6B (TensorRT) | <0.5s |

### Battery and Thermal Impact

**Desktop:** Use the largest/fastest model your hardware supports.

**Laptop (plugged in):** Same as desktop, but consider `gpu_isolation = true`.

**Laptop (battery):**
```toml
[whisper]
model = "small.en"
language = "en"
on_demand_loading = true
gpu_isolation = true
```

---

## Part 5: Language Support Reference

### Whisper: 99+ Languages

Whisper supports most world languages. This is the only option for:

| Region | Languages |
|--------|-----------|
| **East Asia** | Japanese, Chinese (Mandarin, Cantonese), Korean, Vietnamese, Thai |
| **South Asia** | Hindi, Bengali, Tamil, Urdu, Punjabi, Marathi, Gujarati |
| **Middle East** | Arabic, Hebrew, Persian (Farsi) |
| **Africa** | Swahili, Yoruba, Amharic, Hausa, Zulu |
| **Southeast Asia** | Indonesian, Malay, Tagalog, Burmese, Khmer |

Full list: https://github.com/openai/whisper#available-models-and-languages

### Parakeet v3: 25 European Languages

- English, German, French, Spanish, Italian
- Dutch, Polish, Portuguese, Romanian
- Czech, Hungarian, Slovak, Slovenian
- Danish, Norwegian, Swedish, Finnish
- Greek, Turkish, Ukrainian, Russian
- Catalan, Galician, Basque

---

## Part 6: Troubleshooting Model Selection

### "My transcription is slow"

1. **Try Parakeet:** Even on CPU, Parakeet is often faster than Whisper for European languages
2. **Check model size:** Switch to a smaller model
3. **Enable GPU:** Build with appropriate GPU features

### "My transcription has errors"

1. **Try a larger model:** Upgrade from tiny→base→small→medium→large-v3-turbo
2. **Use .en model for English:** English-only models are more accurate
3. **Check language setting:** Ensure `language` matches your speech
4. **Try Parakeet:** It has state-of-the-art accuracy for European languages

### "My language isn't working with Parakeet"

If you're trying to transcribe Japanese, Chinese, Arabic, Hindi, or another non-European language, Parakeet doesn't support it. Switch to Whisper:

```toml
# Remove or comment out the parakeet config
# engine = "parakeet"

[whisper]
model = "large-v3-turbo"
language = "auto"  # or your specific language code
```

### "My laptop gets hot / battery drains"

1. **Enable on-demand loading:** `on_demand_loading = true`
2. **Enable GPU isolation:** `gpu_isolation = true`
3. **Use smaller model:** small.en is efficient
4. **Use CPU inference:** Avoid GPU builds on battery

### "I need punctuation but don't have it"

1. **Use Parakeet TDT:** Includes punctuation automatically (European languages only)
2. **Use post-processing:** Configure LLM cleanup in `[output.post_process]`
3. **Enable spoken punctuation:** `[text] spoken_punctuation = true`

---

## Summary: Recommended Configurations

### Non-European Languages

**If you speak Japanese, Chinese, Korean, Arabic, Hindi, Bengali, Tamil, Vietnamese, Indonesian, Thai, Persian, Hebrew, Swahili, Tagalog, or any other non-European language, Parakeet will not work for you. Use Whisper instead.**

**With GPU:**
```toml
[whisper]
model = "large-v3-turbo"
language = "auto"  # or specify: "ja", "zh", "ko", "ar", "hi", etc.
```
Build: `cargo build --release --features gpu-cuda` (NVIDIA) or `gpu-vulkan` (AMD)

**CPU only:**
```toml
[whisper]
model = "small"      # or "base" for faster but less accurate
language = "auto"    # or specify your language code
```
Build: `cargo build --release`

### Desktop with NVIDIA GPU (English/European)

```toml
engine = "parakeet"

[parakeet]
model = "parakeet-tdt-0.6b-v3"
```
Build: `cargo build --release --features parakeet-cuda`

### Desktop with AMD GPU (English/European)

Option A - Parakeet with ROCm:
```toml
engine = "parakeet"

[parakeet]
model = "parakeet-tdt-0.6b-v3"
```
Build: `cargo build --release --features parakeet-rocm`

Option B - Whisper with Vulkan:
```toml
[whisper]
model = "large-v3-turbo"
language = "en"
```
Build: `cargo build --release --features gpu-vulkan`

### CPU-Only System (English/European)

Option A - Parakeet (faster, has punctuation):
```toml
engine = "parakeet"

[parakeet]
model = "parakeet-tdt-0.6b-v3"
```
Build: `cargo build --release --features parakeet`

Option B - Whisper (more model size options):
```toml
[whisper]
model = "small.en"
language = "en"
```
Build: `cargo build --release`

### Battery-Conscious Laptop

```toml
[whisper]
model = "small.en"
language = "en"
on_demand_loading = true
gpu_isolation = true
```
Build: `cargo build --release`

---

## Further Reading

- [Whisper Model Card](https://github.com/openai/whisper)
- [Whisper Large V3 Turbo](https://huggingface.co/openai/whisper-large-v3-turbo)
- [Parakeet TDT 0.6B v3](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3)
- [NVIDIA Speech AI Blog](https://developer.nvidia.com/blog/nvidia-speech-ai-models-deliver-industry-leading-accuracy-and-performance/)
- [HuggingFace Open ASR Leaderboard](https://huggingface.co/spaces/hf-audio/open_asr_leaderboard)
