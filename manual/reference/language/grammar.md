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
         STRING(보간 조각 포함) PLAIN_STRING(보간 없는 STRING) RAW_STRING SYMBOL NEWLINE, 예약어, 연산자.
   쉼표로 나눈 목록은 모두 끝 쉼표를 받는다. *)

program     = body .
body        = { sep } { stmt { sep } } .
sep         = NEWLINE | ";" .
block       = ":" body ";;" .

stmt        = fn_def | import | type_alias | test_block | assert | assign | expr .
fn_def      = "fn" NAME "(" [ params ] ")" [ "->" type ] fn_body .
fn_body     = "=" expr | block .
params      = ( param { "," param } [ "," rest ] | rest ) [ "," ] .
param       = NAME [ ":" type ] [ "=" expr ] | param_pat .  (* 기본값 뒤의 필수 매개변수는 E0120 *)
rest        = "..." NAME [ ":" type ] .              (* 나머지 매개변수는 맨 끝 하나, 기본값 없음 *)
param_pat   = destructure .                         (* 첫 토큰이 _ ( [ { TAG 인 것 *)
import      = "import" ( NAME { "." NAME } | PLAIN_STRING ) [ "as" NAME ]
              [ "(" NAME { "," NAME } [ "," ] ")" ] .
                                                    (* 최상위만(E0311), 다른 문장보다 먼저 쓴 순서대로.
                                                       목록만 쓰면 모듈 이름은 묶이지 않고, as와 목록은 둘 다 묶는다 *)
type_alias  = "type" NAME "=" type .
test_block  = "test" PLAIN_STRING block .
assert      = "assert" expr .
assign      = destructure "=" expr .                (* 문장 시작에서 `pattern =`를 먼저 시도 *)

expr        = lambda | then_expr .
lambda      = ( NAME | "_" | "(" [ lparam { "," lparam } [ "," ] ] ")" ) "=>" expr .
lparam      = destructure .                         (* fn의 매개변수와 같은 패턴, 기본값·타입·...rest 없음 *)
then_expr   = or_expr [ "then" expr "else" expr ] . (* else는 필수, 오른쪽 결합 *)
or_expr     = and_expr { "or" and_expr } .
and_expr    = not_expr { "and" not_expr } .
not_expr    = "not" not_expr | cmp_expr .
cmp_expr    = pipe_expr [ cmp_op pipe_expr ] .      (* 연쇄 없음 *)
cmp_op      = "==" | "!=" | "<" | ">" | "<=" | ">=" | "in" | "not" "in" .
pipe_expr   = coalesce { "|>" coalesce } .            (* 괄호 없는 호출·태그만 값을 맨 앞 인자로 받는다 *)
coalesce    = range { "??" range } .
range       = add [ ( ".." | "..=" ) add ] .        (* 범위는 범위의 피연산자가 아니다: E0118 *)
add         = mul { ( "+" | "-" ) mul } .
mul         = unary { ( "*" | "/" | "//" | "%" ) unary } .
unary       = "-" unary | pow .
pow         = postfix [ "**" unary ] .              (* 오른쪽 결합, -2 ** 2 = -(2 ** 2) *)
postfix     = primary { call | index | "." key_name | "!" | "?" } .
key_name    = NAME | TAG | KEYWORD .                (* 이름 자리에서는 예약어도 이름 *)
call        = "(" [ arg { "," arg } [ "," ] ] ")" .
arg         = NAME ":" expr | expr | "_" .          (* 이름 붙인 인자, 부분 적용 자리(이름 인자에는 없음) *)
index       = "[" ( expr | [ expr ] ".." [ expr ] | [ expr ] "..=" expr ) "]" .
                                                    (* `[` 바로 안의 `..`·`..=`만 슬라이스, 두 끝은 ??·|>까지:
                                                       xs[a ?? 0..b ?? 2]. 끝의 맨 위가 비교·in·not·and·or·then이면 E0125.
                                                       괄호 친 범위, if 조건·match 대상·for 순회 대상·then 가지의 범위는 범위 값 *)

primary     = INT | FLOAT | STRING | RAW_STRING | DURATION | SYMBOL
            | "true" | "false" | "nil" | NAME | TAG
            | "(" expr ")" | tuple | list | map             (* 괄호는 트리에 남는 묶음이다 *)
            | if_expr | match_expr | for_expr
            | "return" [ expr ]
            | "fn" "(" [ params ] ")" [ "->" type ] block .  (* 익명 함수는 블록 형태만 *)
tuple       = "(" ")" | "(" expr "," [ expr { "," expr } [ "," ] ] ")" .
list        = "[" [ expr { "," expr } [ "," ] ] "]" .
map         = "{" [ entry { "," entry } [ "," ] ] "}" .
entry       = ( key_name | PLAIN_STRING ) ":" expr
            | NAME .                                (* {a, b} = {a: a, b: b}; 대문자 축약은 E0225 *)
if_expr     = "if" expr ":" body { "else" "if" expr ":" body } [ "else" ":" body ] ";;" .
match_expr  = "match" expr ":" { sep } arm { arm } ";;" .
arm         = pattern [ "when" expr ] "=>" { sep } stmt { sep } { stmt { sep } } .
                                                    (* 절은 하나 이상, 절의 본문은 비지 않는다 *)
for_expr    = "for" destructure "in" expr ":" body ";;" .

pattern     = pat_term { "|" pat_term } .
pat_term    = "_" | NAME | [ "-" ] ( INT | FLOAT | DURATION ) | PLAIN_STRING | SYMBOL
            | "true" | "false" | "nil"
            | TAG [ "(" pattern { "," pattern } [ "," ] ")" ]          (* Red()는 E0117 *)
            | "(" ")" | "(" pattern "," [ pattern { "," pattern } [ "," ] ] ")"
            | "[" [ pat_item { "," pat_item } [ "," ] ] "]"
            | "{" [ pat_entry { "," pat_entry } [ "," ] ] "}" .
pat_item    = pattern | "..." [ NAME | "_" ] .       (* ...는 맨 끝에만 *)
pat_entry   = ( key_name | PLAIN_STRING ) ":" pattern | NAME .   (* {line, col}; 대문자 축약은 E0225 *)

destructure = d_term { "|" d_term } .               (* 대입·for·매개변수의 패턴: 맨 바깥에 리터럴 없음(E0112) *)
d_term      = "_" | NAME | TAG "(" pattern { "," pattern } [ "," ] ")"
            | "(" ")" | "(" pattern "," [ pattern { "," pattern } [ "," ] ] ")"
            | "[" [ pat_item { "," pat_item } [ "," ] ] "]"
            | "{" [ pat_entry { "," pat_entry } [ "," ] ] "}" .  (* 안쪽은 어떤 패턴이든: (1, y) = p *)

type        = type_term { "|" type_term } .
type_term   = ( NAME | "nil" | "fn" ) [ "[" type "]" ] | TAG .
```

</details>
