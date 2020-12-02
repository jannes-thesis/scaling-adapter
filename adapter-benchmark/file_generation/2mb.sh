#!/bin/bash
output_dir=$1
n=$2
for (( i=0; i<$n; i++ ))
do
    base64 /dev/urandom | head -c 2097152  > "${output_dir}/2mb-${i}.txt"
done