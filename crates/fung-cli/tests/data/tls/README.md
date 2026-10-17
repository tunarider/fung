# 시험용 TLS 키와 인증서

이 디렉터리의 키(`*.key`)와 인증서(`*.pem`)는 **시험 전용**이다. 4d단계의 TLS·HTTPS 시험
(`crates/fung-cli/tests/tls.rs`, `http_parity.rs`)이 `SSL_CERT_FILE`로 `ca.pem`을 믿게 해서 쓴다.

- 만드는 법: `scripts/gen-test-certs.sh` (각 파일의 용도도 그 스크립트 머리말에 있다). CA 키는 만들고 바로 버린다.
- 개인 키가 공개 저장소에 그대로 있으므로 비밀이 아니다. 시험 밖에서 이 CA나 인증서를 **절대 믿게 하지 않는다**
  (시스템 신뢰 저장소, `SSL_CERT_FILE`, 브라우저 등에 넣지 않는다).
