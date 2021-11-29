#!/bin/bash
openssl req -x509 -subj "/CN=$1"  -addext "subjectAltName = otherName:1.3.6.1.5.5.7.8.11;IA5STRING:dtn://$1" -newkey rsa:2048 -keyout "$1_key.pem" -out "$1_cert.der" -outform der -nodes -days 365
openssl rsa -in "$1_key.pem" -outform der -out "$1_key.der"
rm -f "$1_key.pem"
