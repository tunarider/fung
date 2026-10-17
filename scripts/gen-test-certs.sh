#!/bin/sh
# Make the TLS test certificates (stage 4d) in crates/fung-cli/tests/data/tls.
#
#   scripts/gen-test-certs.sh [outdir]
#
# Writes, all ECDSA P-256:
#   ca.pem            test CA (trusted by the tests through SSL_CERT_FILE)
#   other-ca.pem      a second CA that signed nothing the tests serve
#   localhost.pem/.key   server cert for localhost and 127.0.0.1 (SAN), CA-signed
#   wrongname.pem/.key   server cert for wrong.example only, CA-signed
#   expired.pem/.key     server cert for localhost, valid 2000-01-01 to 2001-01-01
# Each server .pem holds the leaf followed by the CA (the chain a server sends).
# The good certificates are valid for 100 years. Needs openssl (OpenSSL 1.1+,
# 3.x or LibreSSL); the CA keys are thrown away.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
out=${1:-$root/crates/fung-cli/tests/data/tls}
mkdir -p "$out"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

days=36500
ec() { openssl ecparam -name prime256v1 -genkey -noout -out "$1"; }

ca() { # name subject
	ec "$work/$1.key"
	cat >"$work/$1.ext" <<EOF
[req]
distinguished_name = dn
x509_extensions = v3
prompt = no
[dn]
CN = $2
[v3]
basicConstraints = critical, CA:TRUE
keyUsage = critical, keyCertSign, cRLSign
subjectKeyIdentifier = hash
EOF
	openssl req -x509 -new -key "$work/$1.key" -sha256 -days "$days" \
		-config "$work/$1.ext" -out "$out/$1.pem"
}

ca ca "fung test CA"
ca other-ca "fung other test CA"

# leaf name san [startdate enddate]
leaf() {
	ec "$work/$1.key"
	openssl req -new -key "$work/$1.key" -subj "/CN=$2" -out "$work/$1.csr"
	cat >"$work/$1.ext" <<EOF
basicConstraints = critical, CA:FALSE
keyUsage = critical, digitalSignature
extendedKeyUsage = serverAuth
subjectAltName = $3
authorityKeyIdentifier = keyid
subjectKeyIdentifier = hash
EOF
	if [ $# -ge 5 ]; then
		# `openssl ca` is the portable way to set dates in the past
		mkdir -p "$work/db"
		: >"$work/db/index.txt"
		echo 1000 >"$work/db/serial"
		cat >"$work/ca.cnf" <<EOF
[ca]
default_ca = test
[test]
dir = $work/db
database = $work/db/index.txt
serial = $work/db/serial
new_certs_dir = $work/db
default_md = sha256
policy = any
copy_extensions = none
unique_subject = no
[any]
commonName = supplied
EOF
		openssl ca -batch -config "$work/ca.cnf" -cert "$out/ca.pem" -keyfile "$work/ca.key" \
			-in "$work/$1.csr" -startdate "$4" -enddate "$5" -extfile "$work/$1.ext" \
			-notext -out "$work/$1.crt" 2>/dev/null
	else
		openssl x509 -req -in "$work/$1.csr" -CA "$out/ca.pem" -CAkey "$work/ca.key" \
			-set_serial "0x$(openssl rand -hex 8)" -sha256 -days "$days" \
			-extfile "$work/$1.ext" -out "$work/$1.crt"
	fi
	cat "$work/$1.crt" "$out/ca.pem" >"$out/$1.pem"
	# PKCS#8, which every TLS library reads
	openssl pkcs8 -topk8 -nocrypt -in "$work/$1.key" -out "$out/$1.key"
}

leaf localhost localhost "DNS:localhost, IP:127.0.0.1"
leaf wrongname wrong.example "DNS:wrong.example"
leaf expired localhost "DNS:localhost, IP:127.0.0.1" 20000101000000Z 20010101000000Z

echo "wrote $out"
