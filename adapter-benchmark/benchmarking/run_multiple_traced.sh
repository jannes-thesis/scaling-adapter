#!/bin/bash
echo "RUNNING WRITE SSD"
python run_benchmark_traced.py static-load-write-ssd
echo "RUNNING READ SSD"
python run_benchmark_traced.py static-load-read-ssd
echo "RUNNING READ HDD"
python run_benchmark_traced.py static-load-read-hdd
echo "RUNNING WRITE HDD"
python run_benchmark_traced.py static-load-write-hdd
