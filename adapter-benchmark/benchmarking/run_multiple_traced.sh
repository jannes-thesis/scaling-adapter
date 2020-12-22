#!/bin/bash
# echo "RUNNING 1mb 100ms"
# python run_benchmark_traced.py rw_buf_1mb_100ms
# echo "RUNNING 1mb 200ms"
# python run_benchmark_traced.py rw_buf_1mb_200ms
# echo "RUNNING 4kb 100us"
# python run_benchmark_traced.py rw_4kb_100us
# echo "RUNNING 4kb 1ms"
# python run_benchmark_traced.py rw_4kb_1ms
# echo "RUNNING 1mb 50ms"
# python run_benchmark_traced.py rw_buf_1mb_50ms
# echo "RUNNING 100kb 100us"
# python run_benchmark_traced.py rw_100kb_100us
# echo "RUNNING 100kb 1ms"
# python run_benchmark_traced.py rw_100kb_1ms
# echo "RUNNING 4kb 100us with background disk writer"
# python run_benchmark_traced.py rw_4kb_100us-bg_load
# echo "RUNNING 1mb 100ms with background disk writer"
# python run_benchmark_traced.py rw_buf_1mb_100ms-bg_load
# echo "RUNNING 1mb non-buf 1ms"
# python run_benchmark_traced.py rw_1mb_1ms
# echo "RUNNING 2mb buf 100ms"
# python run_benchmark_traced.py rw_buf_2mb_100ms
# echo "RUNNING 4kb 100us with background disk writer 2"
# python run_benchmark_traced.py rw_4kb_100us-bg_load2
# echo "RUNNING 1mb 100ms with background disk writer 2"
# python run_benchmark_traced.py rw_buf_1mb_100ms-bg_load2
echo "RUNNING 2mb read oneshot"
python run_benchmark_traced.py read_2mb_oneshot
echo "RUNNING 2mb rw sync oneshot"
python run_benchmark_traced.py rw_2mb_oneshot
echo "RUNNING 2mb rw nosync oneshot"
python run_benchmark_traced.py rw_nosync_2mb_oneshot
