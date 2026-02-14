#!/bin/bash
SCRIPT_DIR="$(realpath "${BASH_SOURCE%/*}")"
DISK_DIR="$SCRIPT_DIR/yocto/build/disks"
mkdir -p "$DISK_DIR"
DISK_OPTS=""
for i in 0 1 2 3 4; do
    DISK="$DISK_DIR/vdisk${i}.qcow2"
    [ -f "$DISK" ] || qemu-img create -f qcow2 "$DISK" 1G
    DISK_OPTS="$DISK_OPTS -drive file=$DISK,if=virtio,format=qcow2"
done

cd "$SCRIPT_DIR/yocto/build" && . ../poky/oe-init-build-env . > /dev/null && runqemu nographic serialstdio slirp qemuparams="$DISK_OPTS" "$@"
