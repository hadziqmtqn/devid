#!/bin/sh
# S1: a leaf for <name>.test and *.<name>.test, shaped like ca.rs's leaves (ECDSA P-256,
# serverAuth, 397 days), signed by the Devid CA in $CA_DIR (read in place, never copied).
# Usage: cert.sh <name> <out-dir>
set -eu
name=$1 out=$2
CA_DIR=${CA_DIR:-$HOME/.devid/ca}
mkdir -p "$out"
umask 077
openssl ecparam -name prime256v1 -genkey -noout -out "$out/$name.test.key"
cat > "$out/$name.ext" <<X
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature
extendedKeyUsage=serverAuth
subjectAltName=DNS:$name.test,DNS:*.$name.test
authorityKeyIdentifier=keyid
subjectKeyIdentifier=hash
X
openssl req -new -key "$out/$name.test.key" -subj "/CN=$name.test" -out "$out/$name.csr"
openssl x509 -req -in "$out/$name.csr" -CA "$CA_DIR/devid-ca.crt" -CAkey "$CA_DIR/devid-ca.key" \
  -set_serial "0x$(openssl rand -hex 12)" -days 397 -sha256 -extfile "$out/$name.ext" -out "$out/$name.test.crt" 2>/dev/null
rm -f "$out/$name.csr" "$out/$name.ext"
openssl verify -purpose sslserver -CAfile "$CA_DIR/devid-ca.crt" "$out/$name.test.crt"
