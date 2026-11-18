#!/bin/sh
# 사용법: scripts/check.sh [--release] "<기대 문자열>"...
# 커널을 빌드해 QEMU로 2초간 실행하고, 시리얼 출력에 기대 문자열이 모두 있는지 검사한다.
set -e
profile=debug
flag=
if [ "$1" = "--release" ]; then profile=release; flag=--release; shift; fi
cargo build $flag
out=$(mktemp)
err=$(mktemp)
# 임시 파일은 성공/실패와 무관하게 종료 시 지운다.
trap 'rm -f "$out" "$err"' EXIT
qemu-system-riscv64 -machine virt -bios none -smp "${SMP:-4}" -m 128M \
  -display none -monitor none -serial stdio \
  -kernel "target/riscv64gc-unknown-none-elf/$profile/zero-to-os" >"$out" 2>"$err" </dev/null &
pid=$!
sleep 2
kill "$pid" 2>/dev/null || true
# stdout에는 게스트 시리얼 출력만 내보낸다. QEMU 자체 진단 메시지는 섞이면
# 줄 수 세기나 문자열 검사를 오염시키므로 stderr로 따로 보낸다.
cat "$out"
cat "$err" >&2
for expected in "$@"; do
  grep -qF -- "$expected" "$out" || { echo "없음: $expected" >&2; exit 1; }
done
