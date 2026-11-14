---
description: fung의 EBNF.
lede: fung의 EBNF.
---
# 형식 문법

<p>파서가 그대로 옮긴 EBNF다. 언어의 형식 문법은 이 한 곳에만 있다. 식의 층은 <a href="operators.html#precedence">우선순위 표</a>와 같다.</p>
<details>
<summary>EBNF 펼치기</summary>

```plain
(* 토큰: NAME(Lu가 아닌 식별자 글자나 _로 시작) TAG(일반 범주 Lu로 시작) INT FLOAT DURATION
         STRING(보간 조각 포함) PLAIN_STRING(보간 없는 STRING 또는 RAW_STRING: 보간 없는 문자열이 필요한 자리는 원시 문자열도 받는다) RAW_STRING SYMBOL(' 뒤가 NAME 모양) NEWLINE,
         예약어, 연산자.
   쉼표로 나눈 목록은 모두 끝 쉼표를 받는다. *)

program     = { sep } { import { sep } } { stmt { sep } } .   (* import는 다른 문장보다 먼저: E0313 *)
body        = { sep } { stmt { sep } } .
sep         = NEWLINE | ";" .                       (* 앞 문장의 다음 줄을 `-`로 시작하는 문장은 E0136 *)
block       = ":" body ";;" .

stmt        = fn_def | type_alias | test_block | assert | assign | return | expr .
                                                    (* 블록에서 끝까지 갈 수 없는 문장 뒤의 문장은 E0131 *)
fn_def      = "fn" NAME "(" [ params ] ")" [ "->" type ] fn_body .
fn_body     = "=" branch | block .
params      = ( param { "," param } [ "," rest ] | rest ) [ "," ] .
param       = NAME [ ":" type ] [ "=" expr ] | param_pat .  (* 기본값 뒤의 필수 매개변수는 E0120 *)
rest        = "..." NAME [ ":" type ] .              (* 나머지 매개변수는 맨 끝 하나, 기본값 없음 *)
param_pat   = destructure .                         (* 첫 토큰이 _ ( [ { TAG 인 것 *)
import      = "import" ( NAME { "." NAME } | PLAIN_STRING ) [ "as" NAME ]
              [ "(" NAME { "," NAME } [ "," ] ")" ] .
                                                    (* 파일 맨 앞에만(블록 안은 E0311), 쓴 순서대로.
                                                       목록만 쓰면 모듈 이름은 묶이지 않고, as와 목록은 둘 다 묶는다 *)
type_alias  = "type" NAME "=" type .
test_block  = "test" PLAIN_STRING block .
assert      = "assert" expr .
assign      = destructure "=" expr .                (* 문장 시작에서 `pattern =`를 먼저 시도 *)

expr        = lambda | then_expr .                  (* 연산자의 피연산자 자리에서 괄호 없는 람다는 E0110 *)
lambda      = ( NAME | "_" | "(" [ lparam { "," lparam } [ "," ] ] ")" ) "=>" expr .
                                                    (* 본문 안의 return·?는 E0237 *)
lparam      = destructure .                         (* fn의 매개변수와 같은 패턴, 기본값·타입·...rest 없음 *)
then_expr   = or_expr [ "then" branch "else" branch ] .  (* else는 필수, 오른쪽 결합 *)
branch      = return | expr .
return      = "return" [ expr ] .                   (* 문장과 가지의 끝에만, 다른 자리는 E0132 *)
or_expr     = and_expr { "or" and_expr } .
and_expr    = not_expr { "and" not_expr } .
not_expr    = "not" not_expr | cmp_expr .
cmp_expr    = pipe_expr [ cmp_op pipe_expr ] .      (* 연쇄 없음 *)
cmp_op      = "==" | "!=" | "<" | ">" | "<=" | ">=" | "in" | "not" "in" .
pipe_expr   = coalesce { "|>" coalesce } .          (* 괄호 없는 호출·태그만 값을 맨 앞 인자로 받는다.
                                                       괄호 없는 ??가 |>의 피연산자면 E0127 *)
coalesce    = range { "??" range } .
range       = add [ ( ".." | "..=" ) add ] .        (* 범위는 범위의 피연산자가 아니다: E0118 *)
add         = mul { ( "+" | "-" ) mul } .
mul         = unary { ( "*" | "/" | "//" | "%" ) unary } .
unary       = "-" unary | pow .
pow         = postfix [ "**" unary ] .              (* 오른쪽 결합, -2 ** 2 = -(2 ** 2) *)
postfix     = primary { call | index | "." key_name | "!" | "?" } .
key_name    = NAME | TAG | KEYWORD .                (* 이름 자리에서는 예약어도 이름 *)
call        = "(" [ args ] ")" .
args        = pos_arg { "," pos_arg } [ "," spread ] { "," named } [ "," ]
            | spread { "," named } [ "," ]
            | named { "," named } [ "," ] .
pos_arg     = expr | "_" .                          (* 부분 적용 자리 *)
spread      = "..." expr .                          (* 리스트·범위를 위치 인자로 펼침. 마지막 위치 인자 하나,
                                                       리스트 리터럴·태그에는 없다: E0133 *)
named       = NAME ":" expr .                       (* 이름 붙인 인자(_는 없다) *)
index       = "[" ( expr | [ add ] ".." [ add ] | [ add ] "..=" add ) "]" .
                                                    (* `[` 바로 안의 `..`·`..=`만 슬라이스, 두 끝은 range의 피연산자 층.
                                                       끝의 맨 위가 ??·|>·비교·in·not·and·or·then이면 E0125: xs[(a ?? 0)..].
                                                       괄호 친 범위, if 조건·match 대상·for 순회 대상·then 가지의 범위는 범위 값 *)

primary     = INT | FLOAT | STRING | RAW_STRING | DURATION | SYMBOL
            | "true" | "false" | "nil" | NAME | TAG
            | "(" expr ")" | tuple | list | map             (* 괄호는 트리에 남는 묶음이다 *)
            | if_expr | match_expr | for_expr
            | "fn" "(" [ params ] ")" [ "->" type ] block .  (* 익명 함수는 블록 형태만 *)
tuple       = "(" ")" | "(" expr "," [ expr { "," expr } [ "," ] ] ")" .
list        = "[" [ expr { "," expr } [ "," ] ] "]" .
map         = "{" [ entry { "," entry } [ "," ] ] "}" .
entry       = ( key_name | PLAIN_STRING ) ":" expr
            | NAME .                                (* {a, b} = {a: a, b: b}; 대문자 축약은 E0225 *)
if_expr     = "if" expr ":" body { "else" "if" expr ":" body } [ "else" ":" body ] ";;" .
match_expr  = "match" expr ":" { sep } arm { arm } ";;" .
arm         = pattern [ "when" expr ] "=>" { sep } { stmt { sep } } .
                                                    (* 절은 하나 이상, 절의 본문은 비어도 된다(값으로 쓰면 E0224).
                                                       앞 절의 가드 없는 리터럴과 같은 리터럴 절은 E0235 *)
for_expr    = "for" destructure "in" expr ":" body ";;" .

pattern     = pat_term { "|" pat_term } .
pat_term    = "_" | NAME | [ "-" ] ( INT | FLOAT | DURATION ) | PLAIN_STRING | SYMBOL
            | "true" | "false" | "nil"
            | TAG [ "(" pattern { "," pattern } [ "," ] ")" ]          (* Red()는 E0117, Ok·Err의 값이 하나가 아니면 E0236 *)
            | "(" ")" | "(" pattern "," [ pattern { "," pattern } [ "," ] ] ")"
            | "[" [ pat_item { "," pat_item } [ "," ] ] "]"
            | "{" [ map_item { "," map_item } [ "," ] ] "}" .
pat_item    = pattern | "..." [ NAME ] .             (* ...는 맨 끝에만, 이름 없는 나머지는 ... 하나(..._는 E0112) *)
map_item    = pat_entry | "..." [ NAME ] .           (* ...가 없으면 키가 정확히 적은 것뿐인 맵; ...는 맨 끝에만, ..._는 E0112 *)
pat_entry   = ( key_name | PLAIN_STRING ) ":" pattern | NAME .   (* {line, col}; 대문자 축약은 E0225, `{_}`는 E0112 *)

destructure = d_term { "|" d_term } .               (* 대입·for·매개변수의 패턴: 맨 바깥에 리터럴 없음(E0112) *)
d_term      = "_" | NAME | TAG "(" pattern { "," pattern } [ "," ] ")"
            | "(" ")" | "(" pattern "," [ pattern { "," pattern } [ "," ] ] ")"
            | "[" [ pat_item { "," pat_item } [ "," ] ] "]"
            | "{" [ map_item { "," map_item } [ "," ] ] "}" .  (* 안쪽은 어떤 패턴이든: (1, y) = p *)

type        = type_term { "|" type_term } .
type_term   = NAME | "nil" | "fn" | TAG .            (* 원소 타입은 없다: list[int]는 E0130 *)
```

</details>
