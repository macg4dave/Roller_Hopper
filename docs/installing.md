# Running Network Mapper: system requirements

What a computer needs to run Network Mapper. There is no installer or download yet: the app is
built from source (see [building.md](building.md)), and these are the requirements for the
computer that runs that build.

Network Mapper is still a prototype. It shows a sample network and simulated scans; it does
not scan your network yet.

## The computer

| | Requirement |
| --- | --- |
| Operating system | Windows 11, 64-bit. Linux and macOS are planned but not yet confirmed. |
| Processor | 64-bit x86. A build optimised for AVX2 (most processors from 2015 on) runs only on processors with AVX2. |
| Disk | Space for the app and its `icons` folder, plus the AI model you install (see below). |
| Internet | Only to install an AI model. Nothing else is downloaded and nothing is sent. |

Keep the `icons` folder beside `network-mapper.exe`: the app reads device icons from there.

## The AI model (optional)

Everything works without AI. To use it, open Settings → AI settings and choose Install AI. The
model goes into `%APPDATA%\Network Mapper\models\`.

| Model | Download and disk space |
| --- | --- |
| Qwen3-4B (the main choice) | 2.5 GB |
| Qwen3-0.6B | 0.6 GB |
| SmolLM2-135M | 0.15 GB |

Memory needs are still being measured. Earlier tests needed about 2.3 GB of RAM for Qwen3-0.6B
on the processor, and the larger model needs more.

## Running the AI on an NVIDIA graphics card

The standard build includes CUDA, so it can run the AI on an NVIDIA graphics card. That build
needs all of the following, even if you only use the processor:

- **An NVIDIA graphics card of the same generation as the computer that built it.** A copy
  built on a PC with an RTX 50-series card runs its AI only on RTX 50-series (and other
  compute capability 12.0) cards.
- **A recent NVIDIA graphics driver**, version 580 or later.
- **NVIDIA's CUDA 13 and cuDNN 9 runtime files**, either from a CUDA Toolkit installation (as
  set up for building) or copied beside `network-mapper.exe`:
  `cudart64_13.dll`, `cublas64_13.dll`, `cublasLt64_13.dll`, `curand64_10.dll`,
  `nvrtc64_130_0.dll`, `nvrtc-builtins64_132.dll` and the `cudnn*64_9.dll` files.
  Without `cudart64_13.dll`, Windows will not start the app.
- **Graphics memory:** earlier tests used about 3 GB for Qwen3-0.6B and about 6.4 GB for
  Qwen3-4B. These are being re-measured.

On a computer without these, use a CPU-only build: it needs none of them.
