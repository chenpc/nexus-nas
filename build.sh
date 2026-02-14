#!/bin/bash
SCRIPT_DIR="$(realpath "${BASH_SOURCE%/*}")"

DEV_MODE=0
RUN_AFTER=0
TEST_MODE=0
ARGS=()
for arg in "$@"; do
    case "$arg" in
        --dev) DEV_MODE=1 ;;
        --run) RUN_AFTER=1 ;;
        --test) TEST_MODE=1 ;;
        *) ARGS+=("$arg") ;;
    esac
done

git -C "$SCRIPT_DIR" submodule sync --recursive --quiet
git -C "$SCRIPT_DIR" submodule update --init --recursive --quiet

# oe-init-build-env creates yocto/build/conf/ on first run
cd "$SCRIPT_DIR/yocto" && . poky/oe-init-build-env "$SCRIPT_DIR/yocto/build" > /dev/null

cp "$SCRIPT_DIR/meta-nexus/conf/bblayers.conf" "$SCRIPT_DIR/yocto/build/conf/"
cp "$SCRIPT_DIR/meta-nexus/conf/local.conf" "$SCRIPT_DIR/yocto/build/conf/"

if [ "$DEV_MODE" = "1" ]; then
    echo "*** Dev mode: using local source trees ***"
    cat "$SCRIPT_DIR/meta-nexus/conf/dev.conf" >> "$SCRIPT_DIR/yocto/build/conf/local.conf"
fi

if [ "$TEST_MODE" = "1" ]; then
    cat "$SCRIPT_DIR/meta-nexus/conf/test.conf" >> "$SCRIPT_DIR/yocto/build/conf/local.conf"
    if [ "$RUN_AFTER" = "1" ]; then
        # --test --run: include nexus-test but don't auto-run, QEMU stays alive
        echo "*** Test mode: nexus-test installed, run it manually ***"
    else
        # --test only: auto-run nexus-test, poweroff, exit with test result
        echo "*** Test mode: nexus-test will run and power off ***"
        cat "$SCRIPT_DIR/meta-nexus/conf/test-autorun.conf" >> "$SCRIPT_DIR/yocto/build/conf/local.conf"
    fi
fi

bitbake nexus-image "${ARGS[@]}" || exit $?

if [ "$TEST_MODE" = "1" ]; then
    if [ "$RUN_AFTER" = "1" ]; then
        exec "$SCRIPT_DIR/run-qemu.sh"
    else
        # Run QEMU, filter nexus-test output, extract test exit code
        QEMU_LOG=$(mktemp)
        "$SCRIPT_DIR/run-qemu.sh" 2>&1 | tee "$QEMU_LOG" | grep --line-buffered 'nexus-test'
        EXIT_CODE=$(grep -oP 'TEST_EXIT_CODE=\K[0-9]+' "$QEMU_LOG" | tail -1)
        rm -f "$QEMU_LOG"
        exit "${EXIT_CODE:-1}"
    fi
elif [ "$RUN_AFTER" = "1" ]; then
    exec "$SCRIPT_DIR/run-qemu.sh"
fi
