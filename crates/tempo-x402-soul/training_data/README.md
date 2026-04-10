# Cartridge Training Corpus

Training data for the codegen model. Each JSON file contains verified
(context, code) pairs for encoder-decoder training.

## Format

```json
[
  {
    "context": "Problem spec or test code (encoder input)",
    "code": "Complete compilable Rust source (decoder target)",
    "source": "cartridge/{tier}/{slug}",
    "tier": "tier1",
    "slug": "hello-world"
  }
]
```

## Tiers

- **tier1**: Static responses (HTML, JSON, text)
- **tier2**: Request parsing (routing, methods, params)
- **tier3**: KV state (counters, CRUD, persistence)
- **tier4**: Rich apps (todo, calculator, forms)
- **tier5**: Complex apps (games, dashboards, multi-page)
- **frontend**: Leptos frontend cartridges

## Verification

All examples are compile-verified against `wasm32-unknown-unknown`.
