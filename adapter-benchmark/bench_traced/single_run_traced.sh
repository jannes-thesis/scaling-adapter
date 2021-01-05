#!/bin/bash
pool_size=$1
load_type=$2
worker_function=$3
num_jobs=$4
files_dir=$5
output_prefix=$6

sudo -v
start_millis=`date +%s%3N`
nohup ../../target/release/threadpool single fixed $pool_size $load_type $worker_function $num_jobs $files_dir > /dev/null 2> /dev/null < /dev/null &
staprun_pid=$!
main_pid=$!
echo "benchmark exe pid: ${main_pid}"
sleep 2
# get threads of workers
worker_tids=$(./get_child_tids.sh threadpool worker-)
echo "workers: ${worker_tids}"

set -m
sudo nohup staprun topsysm2.ko "targets_arg=$worker_tids" -o "${output_prefix}-syscalls.txt" > /dev/null 2> /dev/null < /dev/null &
staprun_pid=$!
echo "staprun pid for workers: ${staprun_pid}"
pidstat_lite $main_pid $worker_tids > "${output_prefix}-pidstats.txt" &
pidstat_pid=$!
echo "pidstat pid: ${pidstat_pid}"

wait $main_pid 
end_millis=`date +%s%3N`
sudo kill -INT $staprun_pid
tail --pid=$staprun_pid -f /dev/null
# make sure staprun result file is written to disk
sync
let runtime=$end_millis-$start_millis
echo $runtime > "${output_prefix}-runtime_ms.txt"
