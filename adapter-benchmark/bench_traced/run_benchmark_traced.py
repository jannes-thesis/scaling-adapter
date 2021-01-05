from subprocess import run
import json
import sys
from subprocess import Popen
import signal
from time import sleep
from datetime import datetime
from typing import List, Union
from dataclasses import dataclass
import subprocess
import os


runscript = 'single_run_traced.sh'


@dataclass(frozen=True)
class BenchmarkParameters:
    workload_name: str
    worker_function: str
    amount_files: int
    io_threads: List[int]
    files_dir: str
    background_command: Union[None, str]


def get_bench_params(name):
    with open('benchmarks.json') as f:
        benchmarks_json = json.load(f)
    b = benchmarks_json[name]
    if 'bg_load_command' in b:
        bg_command = b['bg_load_command']
    else:
        bg_command = None
    return BenchmarkParameters(b['workload_name'], b['worker_function'], 
        b['amount_files'], b['worker_threads'], b['files_dir'], bg_command)


def execute_config(params: BenchmarkParameters, amount_workers: int, output_dir: str):
    if params.background_command is not None:
        bg_process = Popen(params.background_command.split(' '))
    prefix = f'{output_dir}/t={amount_workers}'
    run(['sudo', 'clear_page_cache'])
    sleep(1)
    with Popen(['bash', runscript, str(amount_workers), params.workload_name, params.worker_function, 
        str(params.amount_files), params.files_dir, prefix], text=True, stdout=subprocess.PIPE) as proc:
        # while running continously obtain stdout and buffer it
        while proc.poll() is None:
            out, _ = proc.communicate()
            print(out)
    if params.background_command is not None:
        print('sending sigint to bg process')
        bg_process.send_signal(signal.SIGINT)
        print('waiting for bg process to terminate')
        bg_process.wait()


if __name__ == '__main__':

    print('dont forget to run \'sudo -v\' before')
    if len(sys.argv) != 2:
        print('usage: ./run_benchmark.py [benchmark name]')
        exit(1)

    benchmark_name = sys.argv[1]
    b_params = get_bench_params(benchmark_name)

    now = datetime.today().strftime('%Y-%m-%d-%H:%M')
    output_dir = f'run-{benchmark_name}-{now}'
    os.mkdir(output_dir)

    for amount in b_params.io_threads:
        execute_config(b_params, amount, output_dir)

    print('benchmark done')
