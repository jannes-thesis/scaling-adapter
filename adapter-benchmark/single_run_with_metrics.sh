#!/bin/bash
num_threads=$1
input_file=$2
amount_writes=$3
output_prefix=$4

start_millis=`date +%s%3N`
../target/release/no_adapter $input_file out/ $amount_writes $num_threads &
main_pid=$!
echo "main pid: ${main_pid}"
sleep 1
# get threads of workers
worker_tids=$(./get_child_tids.sh no_adapter worker)
echo "workers: ${worker_tids}"

set -m
sudo nohup staprun topsysm2.ko "targets_arg=$worker_tids" -o "${output_prefix}-syscalls.txt" > /dev/null 2> /dev/null < /dev/null &
staprun_pid=$!
echo "staprun pid: ${staprun_pid}"
pidstat_lite $main_pid $worker_pids > "${output_prefix}-pidstats.txt" &
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
