# tempo-x402-app

Rust WASM web application demonstrating all x402 payment features.

## Features

- Connect browser wallet (MetaMask, etc.)
- Make paid API requests
- Register endpoints on gateway
- View transaction history

## Development

### Prerequisites

- Rust with `wasm32-unknown-unknown` target
- [Trunk](https://trunkrs.dev/) for building and serving

```bash
# Install Trunk
cargo install trunk

# Add WASM target
rustup target add wasm32-unknown-unknown
```

### Build

```bash
# Development build with hot reload
trunk serve

# Production build
trunk build --release
```

### Deploy

The built files in `dist/` can be deployed to any static hosting:
- Vercel
- Netlify
- GitHub Pages
- CloudFlare Pages

## Architecture

- **Leptos** - Rust WASM UI framework
- **gloo-net** - HTTP client for WASM
- **wasm-bindgen** - JS interop for wallet connection

## License

MIT
