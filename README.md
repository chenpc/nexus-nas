# nexus-nas

Umbrella repository for the Nexus project — a Rust gRPC microservices framework with macro-driven command dispatch, targeting embedded Linux (Yocto).

## Repository Structure

```
nexus-nas/
├── libnexus/              # Core framework library
├── cli-shell/             # Interactive CLI client
├── storage-daemon/        # Example gRPC server
├── meta-nexus/            # Yocto meta-layer
└── yocto/
    ├── poky/              # Yocto base system
    ├── meta-openembedded/ # OpenEmbedded layers
    └── meta-rust-bin/     # Rust toolchain for Yocto
```

## Submodule Relationships

```
nexus-nas (this repo)
│
├── libnexus ─────────────────── git@github.com:chenpc/libnexus.git
│   └── nexus-derive/           (proc macro crate, path dependency inside libnexus)
│
├── storage-daemon ───────────── git@github.com:chenpc/storage-daemon.git
│   └── depends on: libnexus    (git dependency)
│
├── cli-shell ────────────────── git@github.com:chenpc/cli-shell.git
│   └── depends on: libnexus    (git dependency)
│
├── meta-nexus ───────────────── git@github.com:chenpc/meta-nexus.git
│   └── builds: storage-daemon  (Yocto recipe fetches storage-daemon + libnexus)
│
└── yocto/
    ├── poky ─────────────────── https://git.yoctoproject.org/poky (scarthgap)
    ├── meta-openembedded ────── https://github.com/openembedded/meta-openembedded.git (scarthgap)
    └── meta-rust-bin ────────── https://github.com/rust-embedded/meta-rust-bin.git (master)
```

## Dependency Graph

```
                ┌────────────┐
                │  libnexus  │
                │  (library) │
                └─────┬──────┘
                      │ (Cargo git dep)
            ┌─────────┴─────────┐
            ▼                   ▼
   ┌────────────────┐  ┌───────────┐
   │ storage-daemon │  │ cli-shell │
   │    (server)    │  │  (client) │
   └────────┬───────┘  └───────────┘
            │ (Yocto recipe)
            ▼
     ┌────────────┐     ┌───────────────────────────────────┐
     │ meta-nexus │────▶│ yocto/poky + meta-oe + meta-rust  │
     │  (layer)   │     │         (build system)             │
     └────────────┘     └───────────────────────────────────┘
```

## Quick Start

```bash
# Clone with all submodules
git clone --recurse-submodules git@github.com:chenpc/nexus-nas.git

# Or init submodules after clone
git submodule update --init --recursive
```
