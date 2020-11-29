#!/bin/bash
output_dir=$1
n=$2
for (( i=0; i<$n; i++ ))
do
    base64 /dev/urandom | head -c 1048576  > "${output_dir}/1mb-${i}.txt"
done