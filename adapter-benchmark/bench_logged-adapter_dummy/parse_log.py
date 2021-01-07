import json
import os
from datetime import datetime, timedelta

ADAPTER_INIT_MARKER = '_I_AdapterInit'
POOL_SIZE_MARKER = '_I_PSIZE:'
QUEUE_SIZE_MARKER = '_I_QSIZE:'
M1_MARKER = '_I_M1_VAL:'
M2_MARKER = '_I_M2_VAL:'
M1_STDDEV_MARKER = '_I_M1_STDDEV:'
M2_STDDEV_MARKER = '_I_M2_STDDEV:'


def get_log_line_time(line: str) -> datetime:
    line = line.split(' ')[0][1:]
    date_str, time_str = line.split('T')
    year, month, day = date_str.split('-')
    hour, minute, second_ms = time_str.split(':')
    second, millisecond = second_ms.split('.')
    millisecond = millisecond[:-1]
    return datetime(int(year), int(month), int(day), int(hour), int(minute),
                    int(second), int(millisecond) * 1000)


def get_log_line_timestamp_millis(line: str, start_time: datetime) -> int:
    time = get_log_line_time(line)
    millis_since_start = (time - start_time) / timedelta(microseconds=1000)
    return int(millis_since_start)


def convert_pool_size_line(line: str, start_time: datetime) -> tuple[int, int]:
    """ return timestamp, pool size pair"""
    timestamp = get_log_line_timestamp_millis(line, start_time)
    pool_size = int(line.split(POOL_SIZE_MARKER)[1])
    return timestamp, pool_size


def convert_queue_size_line(line: str, start_time: datetime) -> tuple[int, int]:
    """ return timestamp, queue size pair"""
    timestamp = get_log_line_timestamp_millis(line, start_time)
    queue_size = int(line.split(QUEUE_SIZE_MARKER)[1])
    return timestamp, queue_size


def convert_metric_one_line(line: str, start_time: datetime) -> tuple[int, float]:
    """ return timestamp, metric one value pair"""
    timestamp = get_log_line_timestamp_millis(line, start_time)
    metric_one = float(line.split(M1_MARKER)[1])
    return timestamp, metric_one


def convert_metric_two_line(line: str, start_time: datetime) -> tuple[int, float]:
    """ return timestamp, metric two value pair"""
    timestamp = get_log_line_timestamp_millis(line, start_time)
    metric_two = float(line.split(M2_MARKER)[1])
    return timestamp, metric_two


def convert_metric_onesd_line(line: str, start_time: datetime) -> tuple[int, float]:
    """ return timestamp, metric one value pair"""
    timestamp = get_log_line_timestamp_millis(line, start_time)
    metric_one = float(line.split(M1_STDDEV_MARKER)[1])
    return timestamp, metric_one


def convert_metric_twosd_line(line: str, start_time: datetime) -> tuple[int, float]:
    """ return timestamp, metric two value pair"""
    timestamp = get_log_line_timestamp_millis(line, start_time)
    metric_two = float(line.split(M2_STDDEV_MARKER)[1])
    return timestamp, metric_two


def log_to_avg_pool_size(log: dict[str, list[tuple]]) -> float:
    pool_sizes = [tpl[1] for tpl in log['pool_size']]
    if len(pool_sizes) == 0:
        return 0.0
    return sum(pool_sizes) / len(pool_sizes)


def log_to_total_thread_creates(log: dict[str, list[tuple]]) -> int:
    pool_sizes = [tpl[1] for tpl in log['pool_size']]
    last = 0
    total = 0
    for size in pool_sizes:
        total += max(0, size - last)
        last = size
    return total


def parse_result(log_path: str) -> dict[str, list[tuple]]:
    """ get timeseries for pool size, queue size, metric one, metric two """
    with open(log_path) as f:
        log_lines = f.readlines()
    adapter_init_line = next(
        (line for line in log_lines if ADAPTER_INIT_MARKER in line))
    pool_size_lines = [line for line in log_lines if POOL_SIZE_MARKER in line]
    queue_size_lines = [
        line for line in log_lines if QUEUE_SIZE_MARKER in line
    ]
    metric_one_lines = [
        line for line in log_lines if M1_MARKER in line
    ]
    metric_two_lines = [
        line for line in log_lines if M2_MARKER in line
    ]
    metric_onesd_lines = [
        line for line in log_lines if M1_STDDEV_MARKER in line
    ]
    metric_twosd_lines = [
        line for line in log_lines if M2_STDDEV_MARKER in line
    ]

    start_time = get_log_line_time(adapter_init_line)
    timestamp_pool_size_tuples = [convert_pool_size_line(line, start_time) for line in pool_size_lines]
    timestamp_queue_size_tuples = [convert_queue_size_line(line, start_time) for line in queue_size_lines]
    timestamp_metric_one_tuples = [convert_metric_one_line(line, start_time) for line in metric_one_lines]
    timestamp_metric_two_tuples = [convert_metric_two_line(line, start_time) for line in metric_two_lines]
    timestamp_metric_onesd_tuples = [convert_metric_onesd_line(line, start_time) for line in metric_onesd_lines]
    timestamp_metric_twosd_tuples = [convert_metric_twosd_line(line, start_time) for line in metric_twosd_lines]
    return {'pool_size': timestamp_pool_size_tuples, 'queue_size': timestamp_queue_size_tuples,
            'metric_one': timestamp_metric_one_tuples, 'metric_two': timestamp_metric_two_tuples,
            'metric_one_sd': timestamp_metric_onesd_tuples, 'metric_two_sd': timestamp_metric_twosd_tuples}
