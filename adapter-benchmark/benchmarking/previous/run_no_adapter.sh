#!/bin/bash
output_static=$1
hyperfine --min-runs 3 \
	--prepare 'sync; echo 3 | sudo tee /proc/sys/vm/drop_caches' \
	--parameter-scan num_threads 1 8 '../target/release/no_adapter out/10mb.txt out/ 10000 {num_threads}' \
	--export-json $output_static
