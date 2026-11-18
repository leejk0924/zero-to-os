# 문제 해결 기록

겪은 문제를 증상, 원인, 해결 순서로 적는다. 같은 문제를 다시 만났을 때 증상으로 찾을 수 있게 증상은 실제 오류 문구로 쓴다.

## 개발 환경

### `gh` 명령이 `HTTP 401: Bad credentials`로 실패한다

- 원인: 셸의 `GITHUB_TOKEN` 환경 변수에 만료된 토큰이 들어 있고, `gh`가 키체인 로그인보다 이 변수를 먼저 쓴다.
- 해결: `env -u GITHUB_TOKEN gh ...`로 실행한다.

### QEMU를 일정 시간만 실행하고 싶은데 `timeout` 명령이 없다

- 원인: macOS에는 `timeout`이 기본으로 없다.
- 해결: 백그라운드로 띄우고 `sleep` 뒤 `kill`한다. `scripts/check.sh`가 이 방식을 쓴다.

### 지금 코드가 어느 권한 레벨(M/S)에서 도는지 확인할 방법이 없다

- 원인: 이 Mac에는 RISC-V용 GDB가 없고, QEMU 모니터의 `info registers`는 권한 레벨을 보여 주지 않는다. RISC-V에는 자기 권한 레벨을 읽는 명령도 없다.
- 해결: M 모드 전용 CSR(`mstatus` 등)을 읽어 본다. S 모드라면 예외가 나고, QEMU를 `-d int -D <로그 파일>`로 실행하면 로그에 `illegal_instruction`이 남는다.
