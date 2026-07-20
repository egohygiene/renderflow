# Installation

Renderflow is distributed through multiple package channels and can also be built from source.

## Requirements

- Rust 1.94+ for source builds
- Pandoc for document rendering
- Tectonic for PDF output
- FFmpeg for audio/image conversion

!!! note
    Package manager installs may already pull some dependencies for you, but the runtime still needs the external tools required by the outputs you choose.

## Cargo

Install from crates.io:

```bash
cargo install renderflow
```

To install from a local checkout instead:

```bash
cargo install --path .
```

## Homebrew

```bash
brew install egohygiene/tap/renderflow
```

If Homebrew refuses to use the third-party tap, trust and tap it explicitly:

```bash
brew trust egohygiene/renderflow
brew tap egohygiene/renderflow https://github.com/egohygiene/renderflow
brew install renderflow
```

## Scoop (Windows)

Renderflow ships a Scoop manifest in `pkg/scoop/renderflow.json`.

```powershell
scoop bucket add egohygiene https://github.com/egohygiene/renderflow
scoop install renderflow
```

## AUR (Arch Linux)

Stable package:

```bash
yay -S renderflow
```

Git package:

```bash
yay -S renderflow-git
```

## Snap

```bash
snap install renderflow --classic
```

## Binary downloads

Prebuilt binaries are published on the [GitHub Releases page](https://github.com/egohygiene/renderflow/releases/latest).

Typical assets include:

- `renderflow-x86_64-unknown-linux-musl`
- `renderflow-x86_64-unknown-linux-gnu`
- `renderflow-aarch64-unknown-linux-gnu`
- `renderflow-aarch64-apple-darwin`
- `renderflow-x86_64-apple-darwin`
- `renderflow-x86_64-pc-windows-msvc.exe`

Download the binary for your platform, place it on your `PATH`, and make it executable on Unix-like systems:

```bash
chmod +x renderflow-*
mv renderflow-* /usr/local/bin/renderflow
```

## From source

```bash
git clone https://github.com/egohygiene/renderflow.git
cd renderflow
cargo build --release
cargo install --path .
```

## Verify the install

```bash
renderflow --version
renderflow --help
```
