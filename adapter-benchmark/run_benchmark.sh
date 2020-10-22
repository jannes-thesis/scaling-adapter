#!/bin/bash
output_static=$1
output_adaptive=$2
hyperfine --min-runs 3 \
	--prepare 'sync; echo 3 | sudo tee /proc/sys/vm/drop_caches' \
	--parameter-scan num_threads 1 8 '../target/release/adapter_thread out/10mb.txt out/ 1000 --static={num_threads}' \
	--export-json $output_static
hyperfine --min-runs 3 \
	--prepare 'sync; echo 3 | sudo tee /proc/sys/vm/drop_caches' \
	'../target/release/adapter_thread out/10mb.txt out/ 1000' \
	--export-json $output_adaptive
