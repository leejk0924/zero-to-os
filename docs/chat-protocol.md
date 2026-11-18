# `.chat/` 에이전트 대화 규약

메인 세션(main), 구현 서브에이전트(impl), Codex(codex)가 지시·보고·결정 요청을 주고받는 형식이다. 에이전트만 읽는다. 짧고 기계적으로 쓴다. 설계와 이유는 `docs/decision/specs/2026-11-19-chat-protocol-design.md`에 있다.

## 위치와 이름

```
.chat/<이슈>-<태스크>/<NNN>-<type>.md     예: .chat/1-2/003-ask.md
```

- `NNN`은 그 폴더 안에서 001부터 차례로 붙인다.
- `.chat/`은 커밋하지 않는다.

## 쓰는 규칙

- 머리말(아래) + `KEY: 값` 줄과 `-` 목록만 쓴다. 문장으로 설명하지 않는다.
- 파일은 새로 만들기만 한다. 예외는 `ask`의 `status`를 `open`에서 `done`으로 바꾸는 것 하나다.
- `status`는 `ask`만 `open`으로 시작한다. 나머지는 `done`.
- 열린 요청 찾기: `grep -rl 'status: open' .chat/`

## 흐름

1. main이 `001-task`를 쓰고 서브에이전트에게 그 경로를 넘긴다.
2. impl이 막히면 `ask`를 쓴다. `BLOCKING: yes`면 멈추고, `no`면 추천대로 진행해 `report`의 `DEVIATION`에 남긴다.
3. main이 문서(`decisions.md`, `intent.md`, 설계·계획)로 답할 수 있으면 `answer`를 쓴다. 사용자만 정할 수 있으면 `docs/decision/qna.md`에 올리고, 답을 받은 뒤 `BASIS: qna.md Q<N>`으로 `answer`를 쓴다. 그리고 `ask`를 `done`으로 바꾼다.
4. impl이 커밋하고 `report`를 쓴다.
5. main이 Codex 출력을 `review`로 저장하고 `triage`를 쓴다. 고친 impl이 다시 `report`를 쓴다.

## 템플릿

### task (main → impl)

```
---
from: main
to: impl
type: task
status: done
refs: []
---
PLAN: docs/decision/plans/<파일>.md Task <N>
BRANCH: <유형>/<이슈>-<기능>
VERIFY:
- <명령> → <기대 결과>
LIMIT:
- <하지 말 것>
```

### report (impl → main)

```
---
from: impl
to: main
type: report
status: done
refs: [<task 또는 triage 번호>]
---
RESULT: ok | blocked
COMMITS:
- <SHA> <제목>
VERIFY:
- <명령> → <실제 결과>
DEVIATION:
- <계획과 다른 점, 없으면 없음>
NOTE:
- <학습 노트에 쓸 사실: 실행 결과, 겪은 문제, 원인>
```

### ask (impl → main)

```
---
from: impl
to: main
type: ask
status: open
refs: [<관련 번호>]
---
Q: <질문>
OPT:
- A: <선택지>
- B: <선택지>
REC: <A | B>
WHY: <추천 이유>
BLOCKING: yes | no
```

### answer (main → impl)

```
---
from: main
to: impl
type: answer
status: done
refs: [<ask 번호>]
---
A: <결정>
BASIS: <D번호 | 설계 문서 절 | qna.md Q<N>>
```

### review (codex → main, main이 저장)

```
---
from: codex
to: main
type: review
status: done
refs: [<report 번호>]
---
COMMIT: <SHA>
<codex exec review --commit <SHA> 출력 그대로>
```

### triage (main → impl)

```
---
from: main
to: impl
type: triage
status: done
refs: [<review 번호>]
---
- <지적 번호>: fix | skip — <이유>
```
