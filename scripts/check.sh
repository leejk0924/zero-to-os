#!/bin/sh
# 사용법: scripts/check.sh [--release] "<기대 문자열>"...
# 커널을 빌드해 QEMU로 2초간 실행하고, 시리얼 출력에 기대 문자열이 모두 있는지 검사한다.
set -e
profile=debug
flag=
if [ "$1" = "--release" ]; then profile=release; flag=--release; shift; fi
cargo build $flag
out=$(mktemp)
qemu-system-riscv64 -machine virt -bios none -smp "${SMP:-4}" -m 128M \
  -display none -monitor none -serial stdio \
  -kernel "target/riscv64gc-unknown-none-elf/$profile/zero-to-os" >"$out" 2>&1 </dev/null &
pid=$!
sleep 2
kill "$pid" 2>/dev/null || true
cat "$out"
for expected in "$@"; do
  grep -qF -- "$expected" "$out" || { echo "없음: $expected" >&2; exit 1; }
done
