#!/bin/bash
SCRIPT_DIR="$(realpath "${BASH_SOURCE%/*}")"

DEV_MODE=0
RUN_AFTER=0
ARGS=()
for arg in "$@"; do
    case "$arg" in
        --dev) DEV_MODE=1 ;;
        --run) RUN_AFTER=1 ;;
        *) ARGS+=("$arg") ;;
    esac
done

cp "$SCRIPT_DIR/meta-nexus/conf/bblayers.conf" "$SCRIPT_DIR/yocto/build/conf/"
cp "$SCRIPT_DIR/meta-nexus/conf/local.conf" "$SCRIPT_DIR/yocto/build/conf/"

if [ "$DEV_MODE" = "1" ]; then
    echo "*** Dev mode: using local source trees ***"
    cat "$SCRIPT_DIR/meta-nexus/conf/dev.conf" >> "$SCRIPT_DIR/yocto/build/conf/local.conf"
fi

cd "$SCRIPT_DIR/yocto/build" && . ../poky/oe-init-build-env . > /dev/null && bitbake nexus-image "${ARGS[@]}" || exit $?

if [ "$RUN_AFTER" = "1" ]; then
    exec "$SCRIPT_DIR/run-qemu.sh"
fi
