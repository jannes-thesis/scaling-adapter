#!/bin/bash
size_in_bytes=$1
output_path=$2
base64 /dev/urandom | head -c $size_in_bytes > $output_path
