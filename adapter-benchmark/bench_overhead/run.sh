#!/bin/bash
load_type="oneshot"
worker_function="read_write_4kb_sync"
num_jobs=100000
files_dir="/ssd2/adapter-benchmark/files"
pool_size=16

params_str="$load_type $worker_function $num_jobs $files_dir"
cmd_no_adapter="../../target/release/threadpool single fixed $pool_size $params_str"
cmd_with_adapter="../../target/release/threadpool single fixed-overhead $pool_size,200 $params_str"

hyperfine --min-runs 5 \
    --style basic \
    --prepare 'sudo /bin/clear_page_cache' \
    "bash ${cmd_no_adapter}" \
    --export-json "result-fixed.json"

hyperfine --min-runs 5 \
    --style basic \
    --prepare 'sudo /bin/clear_page_cache' \
    "bash ${cmd_with_adapter}" \
    --export-json "result-fixed.json"