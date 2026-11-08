---
description: fung의 EBNF.
lede: fung의 EBNF.
---
# 형식 문법

<p>파서가 그대로 옮긴 EBNF다. 식의 층은 위 우선순위 표와 같다.</p>
<details>
<summary>EBNF 펼치기</summary>

```plain
(* 토큰: NAME(소문자·_·대소문자 없는 글자로 시작) TAG(대문자 시작) INT FLOAT DURATION
         STRING(보간 조각 포함) RAW_STRING SYMBOL NEWLINE, 예약어, 연산자 *)

program     = body .
body        = { sep } { stmt { sep } } .
sep         = NEWLINE | ";" .
block       = ":" body ";;" .

stmt        = fn_def | import | type_alias | test_block | assert | assign | expr .
fn_def      = "fn" NAME "(" [ params ] ")" [ "->" type ] fn_body .
fn_body     = "=" expr | block .
params      = param { "," param } [ "," ] .
param       = [ "..." ] NAME [ ":" type ] [ "=" expr ] | pattern .
import      = "import" ( NAME | STRING ) [ "as" NAME ] [ "(" NAME { "," NAME } ")" ] .
type_alias  = "type" NAME "=" type .
test_block  = "test" STRING block .
assert      = "assert" expr .
assign      = pattern "=" expr .                   (* 문장 시작에서 `pattern =`를 먼저 시도 *)

expr        = lambda | then_expr .
lambda      = ( NAME | "_" | "(" [ pattern { "," pattern } ] ")" ) "=>" expr .
then_expr   = or_expr [ "then" expr "else" expr ] .
or_expr     = and_expr { "or" and_expr } .
and_expr    = not_expr { "and" not_expr } .
not_expr    = "not" not_expr | cmp_expr .
cmp_expr    = pipe_expr [ cmp_op pipe_expr ] .      (* 연쇄 없음 *)
cmp_op      = "==" | "!=" | "<" | ">" | "<=" | ">=" | "in" | "not" "in" .
pipe_expr   = coalesce { "|>" coalesce } .
coalesce    = range { "??" range } .
range       = add [ ( ".." | "..=" ) add ] .
add         = mul { ( "+" | "-" ) mul } .
mul         = unary { ( "*" | "/" | "//" | "%" ) unary } .
unary       = "-" unary | pow .
pow         = postfix [ "**" unary ] .              (* 오른쪽 결합 *)
postfix     = primary { call | index | "." key_name | "!" | "?" } .
key_name    = NAME | TAG | KEYWORD .                (* 이름 자리에서는 예약어도 이름 *)
call        = "(" [ arg { "," arg } [ "," ] ] ")" .
arg         = [ NAME ":" ] expr | "_" .
index       = "[" ( expr | [ expr ] ".." [ expr ] | expr "..=" expr ) "]" .

primary     = INT | FLOAT | STRING | RAW_STRING | DURATION | SYMBOL
            | "true" | "false" | "nil" | NAME | TAG
            | "(" expr ")" | tuple | list | map
            | if_expr | match_expr | for_expr
            | "return" [ expr ]
            | "fn" "(" [ params ] ")" [ "->" type ] block .
tuple       = "(" ")" | "(" expr "," [ expr { "," expr } [ "," ] ] ")" .
list        = "[" [ expr { "," expr } [ "," ] ] "]" .
map         = "{" [ entry { "," entry } [ "," ] ] "}" .
entry       = ( key_name | STRING ) ":" expr | NAME | TAG .
if_expr     = "if" expr ":" body { "else" "if" expr ":" body } [ "else" ":" body ] ";;" .
match_expr  = "match" expr ":" { sep } { arm } ";;" .
arm         = pattern [ "when" expr ] "=>" body .
for_expr    = "for" pattern "in" expr ":" body ";;" .

pattern     = pat_term { "|" pat_term } .
pat_term    = "_" | NAME | [ "-" ] ( INT | FLOAT ) | STRING | SYMBOL | DURATION
            | "true" | "false" | "nil"
            | TAG [ "(" [ pattern { "," pattern } ] ")" ]
            | "(" ")" | "(" pattern "," [ pattern { "," pattern } [ "," ] ] ")"
            | "[" [ pattern { "," pattern } ] [ [ "," ] "..." [ NAME ] ] "]"
            | "{" [ pat_entry { "," pat_entry } ] "}" .
pat_entry   = key_name ":" pattern | NAME | TAG .

type        = type_term { "|" type_term } .
type_term   = NAME [ "[" type "]" ] | TAG .
```

</details>
