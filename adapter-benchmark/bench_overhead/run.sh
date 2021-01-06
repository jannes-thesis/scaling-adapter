#!/bin/bash
load_type=$1
worker_function=$2
num_jobs=$3
out_prefix=$4
files_dir="/ssd2/adapter-benchmark/files"
pool_size=16

params_str="$load_type $worker_function $num_jobs $files_dir"
cmd_no_adapter="../../target/release/threadpool single fixed $pool_size $params_str"
cmd_with_adapter="../../target/release/threadpool single fixed-overhead $pool_size $params_str"

hyperfine --min-runs 5 \
    --style basic \
    --prepare 'sudo /bin/clear_page_cache' \
    "${cmd_no_adapter}" \
    --export-json "${out_prefix}-fixed.json"

hyperfine --min-runs 5 \
    --style basic \
    --prepare 'sudo /bin/clear_page_cache' \
    "${cmd_with_adapter}" \
    --export-json "${out_prefix}-overhead.json"