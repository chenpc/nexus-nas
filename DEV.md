# Developer Guide

## Build & Run

```bash
# Build image only
./build.sh --dev

# Build and boot QEMU
./build.sh --dev --run
```

## Testing

```bash
# Build, boot QEMU, run tests, poweroff, exit with test result
./build.sh --dev --test

# Build, boot QEMU, run tests, keep QEMU alive for debugging
./build.sh --dev --test --run
```

| Command | Builds | Boots QEMU | storage-daemon | nexus-test | Powers off |
|---|---|---|---|---|---|
| `--dev` | yes | no | - | - | - |
| `--dev --run` | yes | yes | runs | no | no |
| `--dev --test` | yes | yes | disabled | auto-runs | yes (exit code = test result) |
| `--dev --test --run` | yes | yes | disabled | run manually | no |

### Test binary

`nexus-test` is a standalone binary that tests all services via `Registry::execute` dispatch (no gRPC). Source: `storage-daemon/src/test.rs`.

To run manually inside QEMU:
```
nexus-test
```

### Adding tests

Add new test cases in `storage-daemon/src/test.rs` using the `run!` macro:

```rust
run!(r, reg, "service.command", {
    let resp = rt.block_on(reg.execute("service", "command", vec!["arg".into()]))
        .map_err(|e| e.to_string())?;
    assert_contains!(resp, "expected");
});
```

## Yocto: Custom Kernel Config

```bash
# Generate .config
bitbake linux-yocto -c kernel_configme -f

# menuconfig
bitbake linux-yocto -c menuconfig

# Make diff
bitbake linux-yocto -c diffconfig
```

For `recipes-kernel/linux/linux-yocto_%.bbappend`:
```
FILESEXTRAPATHS:prepend := "${THISDIR}/${PN}:"
SRC_URI += "file://fragment.cfg"
```

## VirtioFS

Host:
```bash
/usr/lib/qemu/virtiofsd --socket-path=/tmp/vm001-vhost-fs.sock --shared-dir `pwd` --tag myfs
```

QEMU:
```bash
runqemu nographic serialstdio slirp snapshot qemuparams="-chardev socket,id=char0,path=/tmp/vm001-vhost-fs.sock \
    -device vhost-user-fs-pci,chardev=char0,tag=myfs \
    -object memory-backend-memfd,id=mem,size=256M,share=on \
    -numa node,memdev=mem"
```

Guest:
```bash
mount -t virtiofs myfs /mnt
```
