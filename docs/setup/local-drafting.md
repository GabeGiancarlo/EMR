# Local clinical drafting runtime

The local generation/review workflow has [verification evidence](../development/knowledge-drafting-checkpoint.md). The full product acceptance ledger remains open. Authentication is deliberately deferred and all services stay on loopback. Do not expose the model service publicly.

## Installation

From the repository in PowerShell:

```powershell
.\tools\setup-local-drafting.ps1
.\tools\start-local-model.ps1
```

Keep the model process running and restart the API with `tools/start-api.ps1`. The helper detects the installed model and configures port 8093 and alias `nexus-qwen3-4b`. It does not claim service health until a generation request reaches the service. Existing model environment variables take precedence. Optional overrides are `NEXUS_LLM_PORT`, `NEXUS_LLM_ALIAS` and the absolute `NEXUS_LLM_MODEL` path; remote URLs are not supported.

The Windows helper installs official llama.cpp release `b12058`, Vulkan x64, and official Qwen3-4B Q4_K_M weights. Downloads total about 2.54 GB. Neither is committed to Git. The model uses local GPU/CPU resources rather than xAI credits. The current machine has an 8-GB RTX 5060 Ti and 16-GB system memory; availability on other machines needs verification.

Pinned artifacts:

- Runtime release: [ggml-org llama.cpp b12058](https://github.com/ggml-org/llama.cpp/releases/tag/b12058), `llama-b12058-bin-win-vulkan-x64.zip`, SHA256 `fbab055f598dcd628f773db531741c64d14c652e7eefb2dfebc6953e8bcaf197` from the release asset digest.
- Model: [official Qwen3-4B-GGUF](https://huggingface.co/Qwen/Qwen3-4B-GGUF/tree/bc640142c66e1fdd12af0bd68f40445458f3869b), `Qwen3-4B-Q4_K_M.gguf`, SHA256 `7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5` from the publisher's LFS object metadata. The model card declares Apache-2.0; redistribution needs the applicable notices.

Checksums are checked before extraction/use; a differing existing file causes an error rather than silent replacement. The setup does not install a Windows service or modify startup/login configuration.

## Service boundary

The configured service uses one slot, an 8,192-token context, non-thinking generation, no web UI and disabled logs. The API uses JSON-constrained chat completions, fixed loopback transport, bounded inputs/output/timeouts, no proxies/redirects, and records model identity/hash. The [llama.cpp server documentation](https://github.com/ggml-org/llama.cpp/blob/b12058/tools/server/README.md) describes its chat-completion endpoint and JSON-schema support. The [Qwen model documentation](https://huggingface.co/Qwen/Qwen3-4B-GGUF) describes the non-thinking option; it does not establish clinical suitability.

The model is a general-purpose model, not a validated clinical decision system. Exact citation checks reject unknown source IDs and quotations absent from the selected evidence. They cannot prove that a paraphrase is medically correct, that a quoted statement applies now, or that source data itself is accurate. Clinicians must review attribution, negation, dates, conclusions and unresolved information. The model cannot sign records, create diagnoses, prescribe or certify delivered care.

No prompts or patient records are sent to xAI. Source-only xAI workers assist development separately. Requests, model outputs and reviewer decisions persist in the local database as clinical drafting history under current chart permissions. The model process holds prompts in memory while working; stopping it releases the process. Production storage/key protection, retention and destruction of derivative records remain separate acceptance work.
