# `.chat/` 에이전트 대화 규약 설계

## 왜

태스크마다 새 서브에이전트가 구현하고 Codex가 검토한다(D19). 지시, 보고, 결정 요청을 대화창 대신 `.chat/`의 파일로 주고받아, 새 서브에이전트에게 필요한 맥락만 넘기고 열린 결정을 기계적으로 찾는다.

- 목표: 누가 쓰든 같은 형식. `grep`으로 열린 요청을 찾을 수 있음.
- 하지 않는 것: 사람이 읽기 위한 설명. 사용자 대상 질문은 계속 `qna.md`, 개념 설명은 학습 노트.

## 버린 대안과 고른 것

| 안 | 내용 | 판단 |
|---|---|---|
| A | 태스크마다 대화 파일 하나에 이어 쓰기 | 새 서브에이전트가 전체를 읽어야 하고, 열린 요청을 찾기 어렵다 |
| **B** | **메시지 하나 = 파일 하나, 머리말에 기계용 필드** | **사용자가 고름.** 필요한 파일만 넘기고 `grep`으로 상태를 찾는다 |
| C | B + 태스크 상태판 `state.md` | 상태를 두 곳에서 맞춰야 해서 과하다 |

## 규약

### 위치와 이름

```
.chat/<이슈>-<태스크>/<NNN>-<type>.md     예: .chat/1-2/003-ask.md
```

- `NNN`은 그 폴더 안에서 001부터 차례로 붙인다.
- `.chat/`은 커밋하지 않는다(`.gitignore`).

### 머리말

```yaml
---
from: main        # main | impl | codex
to: impl          # main | impl
type: task        # task | report | ask | answer | review | triage
status: done      # ask만 open으로 시작, answer가 오면 done으로 바꾼다. 나머지는 done
refs: []          # 이 메시지가 답하는 메시지 번호, 예: [003]
---
```

### 본문

- `KEY: 값` 줄과 `-` 목록만 쓴다. 문장으로 설명하지 않는다.
- 파일은 덧붙이기만 한다. 예외는 `ask`의 `status`를 바꾸는 것 하나다.

| type | 누가 → 누구 | 본문 필드 |
|---|---|---|
| `task` | main → impl | `PLAN:` 계획 파일과 Task 번호 · `BRANCH:` · `VERIFY:` 확인 명령과 기대 결과 · `LIMIT:` 하지 말 것 |
| `report` | impl → main | `RESULT:` ok \| blocked · `COMMITS:` SHA · `VERIFY:` 실제 결과 · `DEVIATION:` 계획과 다른 점 · `NOTE:` 학습 노트에 쓸 사실 |
| `ask` | impl → main | `Q:` · `OPT:` 선택지 · `REC:` 추천 · `WHY:` · `BLOCKING:` yes \| no |
| `answer` | main → impl | `A:` · `BASIS:` 근거 문서(`D6`, 설계 문서 절) 또는 `qna.md Q<N>` |
| `review` | codex → main | `codex exec review` 출력을 그대로 (main이 저장) |
| `triage` | main → impl | 지적마다 `- <번호>: fix \| skip — <이유>` |

### 흐름

1. main이 `001-task`를 쓰고 서브에이전트에게 그 파일 경로를 넘긴다.
2. 서브에이전트가 막히면 `ask`(`status: open`)를 쓰고 멈춘다. `BLOCKING: no`면 추천대로 진행하고 보고에 남긴다.
3. main이 문서로 답할 수 있으면 `answer`를 쓴다. 사용자만 정할 수 있으면 `qna.md`에 올리고, 답을 받은 뒤 `BASIS: qna.md Q<N>`으로 `answer`를 쓴다. `ask`를 `done`으로 바꾼다.
4. 서브에이전트가 커밋하고 `report`를 쓴다.
5. main이 Codex 출력을 `review`로 저장하고 `triage`를 쓴다. 고치는 서브에이전트가 다시 `report`를 쓴다.
6. 열린 요청 찾기: `grep -rl 'status: open' .chat/`

## 구현할 때 정할 것

- 규약 본문은 `docs/chat-protocol.md`로 옮기고 `AGENTS.md`의 문서 표와 구현 단계에서 가리킨다. 이 설계 문서는 기록으로 남긴다.
- 타입별 빈 템플릿을 `docs/chat-protocol.md` 안에 둔다(별도 파일로 나누지 않음).
