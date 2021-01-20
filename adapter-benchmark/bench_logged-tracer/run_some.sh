#!/bin/bash
sudo clear_page_cache
sleep 10
echo "1/4"
RUST_LOG=info ../../target/release/threadpool single inc-tracer 2000,120 oneshot read_write_2mb_sync 5000 /ssd2/adapter-benchmark/files 2> logs/chapter4/inc-tracer_rwsync2mb5k.log
sudo clear_page_cache
sleep 10
echo "2/4"
RUST_LOG=info ../../target/release/threadpool single inc-tracer 4000,120 oneshot read_2mb 10000 /ssd2/adapter-benchmark/files 2> logs/chapter4/inc-tracer_read2mb10k.log
sudo clear_page_cache
sleep 10
echo "3/4"
RUST_LOG=info ../../target/release/threadpool single fixed-tracer 30 oneshot read_write_2mb_sync 5000 /ssd2/adapter-benchmark/files 2> logs/chapter4/fixed-tracer_rwsync2mb5k.log
sudo clear_page_cache
sleep 10
echo "4/4"
RUST_LOG=info ../../target/release/threadpool single fixed-tracer 3 oneshot read_2mb 10000 /ssd2/adapter-benchmark/files 2> logs/chapter4/fixed-tracer_read2mb10k.log

