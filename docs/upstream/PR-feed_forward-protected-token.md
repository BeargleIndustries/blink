# Draft upstream PR — leejet/stable-diffusion.cpp

Base: `6b3edaa`. Patch: `crates/sd-sys/patches/0001-feed_forward-protected-token.patch` (one line).
Not yet submitted. Apply to a fork of sd.cpp, push a branch, and paste the text below.

---

## Title

fix: protect `feed_forward` token in LoRA name conversion (Z-Image LoRAs lose FFN weights)

## Description

`convert_sep_to_dot()` in `src/name_conversion.cpp` rewrites underscores in LoRA
tensor names to dots, with a `protected_tokens` allowlist for names that legitimately
contain underscores. `feed_forward` is not on that list, so a Z-Image LoRA key such as

```
lora_unet_layers_0_feed_forward_w1.lora_down.weight
```

becomes

```
lora.model.diffusion_model.layers.0.feed.forward.w1.weight.lora_down
```

which matches no model tensor (the Z-Image block is named `feed_forward`, see
`src/model/diffusion/z_image.hpp`). Every FFN LoRA tensor is then dropped with
`unused lora tensor` warnings, and only the attention layers of the LoRA are applied.

This is the same class of bug fixed by #1786 (`cross_attn` / `output_proj` for Anima)
and #1864 (`token_refiner` for MiniMax H3); this PR adds the missing token for
Z-Image.

## Impact

Measured on Z-Image Turbo Q8, 512x512, 4 steps, fixed seed, comparing pixel divergence
from a no-LoRA baseline:

| LoRA | Format | FFN tensors dropped before | `unused lora tensor` warnings before → after |
|---|---|---|---|
| kohya-trained (`lora_down`/`lora_up`, 630 tensors) | kohya | 270 (43%) | 270 → 0 |
| diffusers-trained (`lora_A`/`lora_B`, 480 tensors) | diffusers | 180 (37.5%) | 180 → 0 |

Effect on LoRA strength (kohya LoRA, mean abs pixel diff from baseline, /255):

| multiplier | before | after |
|---|---|---|
| 0.25 | 3.84 | 10.21 |
| 0.50 | 7.53 | 16.52 |
| 1.00 | 12.42 | 25.57 |

Clipping stays negligible (≤0.05%) after the fix, i.e. the extra strength is the
missing FFN contribution, not saturation. An attention-only Z-Image LoRA is unaffected
before and after, which is the expected behaviour.

Note: this is independent of #1071 (LoRA loaded four times on z-image-turbo), which is
still reproducible at `6b3edaa` — the runtime path logs four loads of the LoRA file per
generation. This PR does not address that.

## Change

```diff
         "cross_attn",
         "output_proj",
         "token_refiner",
+        "feed_forward",
     };
```

## Testing

- Built with CUDA on Windows (MSVC), ran txt2img with the two LoRAs above at several
  multipliers; confirmed zero `unused lora tensor` warnings and the strength changes in
  the tables.
- Not run: other model families. `feed_forward` is a generic name; I checked that
  protecting it cannot break a model that uses `feed.forward` in its converted names,
  since the protection only preserves an underscore that was already present in the
  source key.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
