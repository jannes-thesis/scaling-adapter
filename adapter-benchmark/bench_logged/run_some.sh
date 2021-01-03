#!/bin/bash
sudo clear_page_cache
# RUST_LOG=info ../../target/release/threadpool single fixed-tracer 16 oneshot read_write_2mb_sync 10000 /ssd2/adapter-benchmark/files 2> fixed-tracer_rwsync2mb10k.log
RUST_LOG=info ../../target/release/threadpool single inc-tracer 2000,64 oneshot read_write_2mb_sync 10000 /ssd2/adapter-benchmark/files 2> inc-tracer2_rwsync2mb10k.log
sudo clear_page_cache
# RUST_LOG=info ../../target/release/threadpool single fixed-tracer 16 oneshot read_write_2mb_nosync 10000 /ssd2/adapter-benchmark/files 2> fixed-tracer_rwnosync2mb10k.log
RUST_LOG=info ../../target/release/threadpool single inc-tracer 2000,64 oneshot read_write_2mb_nosync 10000 /ssd2/adapter-benchmark/files 2> inc-tracer2_rwnosync2mb10k.log

