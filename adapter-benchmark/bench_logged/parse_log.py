from datetime import datetime, timedelta

INIT_MARKER = '_METRICS_init'
QUEUE_SIZE_MARKER = '_METRICS_qsize:'
POOL_SIZE_MARKER = '_METRICS_psize:'
WCHAR_MARKER = '_METRICS_wchar:'
RCHAR_MARKER = '_METRICS_rchar:'
WBYTES_MARKER = '_METRICS_write_bytes:'
RBYTES_MARKER = '_METRICS_read_bytes:'
BLKIO_MARKER = '_METRICS_blkio:'
SYSCTIME_MARKER = '_METRICS_sysc-time:'
SYSCCOUNT_MARKER = '_METRICS_sysc-count:'

METRIC_MARKERS = {'qsize': QUEUE_SIZE_MARKER, 'wchar': WCHAR_MARKER, 'rchar': RCHAR_MARKER,
                  'wbytes': WBYTES_MARKER, 'rbytes': RBYTES_MARKER, 'blkio_delay': BLKIO_MARKER,
                  'syscall_time': SYSCTIME_MARKER, 'syscall_count': SYSCCOUNT_MARKER, 'psize': POOL_SIZE_MARKER}


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


def convert_metric_line(line: str, marker: str, start_time: datetime) -> tuple[int, int]:
    """ return timestamp, metric pair"""
    timestamp = get_log_line_timestamp_millis(line, start_time)
    metric = int(line.split(marker)[1])
    return timestamp, metric


def parse_result(log_path: str) -> dict[str, list[tuple]]:
    """ get timeseries for metrics """
    with open(log_path) as f:
        log_lines = f.readlines()
    init_line = next(
        (line for line in log_lines if INIT_MARKER in line))
    start_time = get_log_line_time(init_line)
    result = {}
    for m in METRIC_MARKERS.keys():
        metric_lines = [
            line for line in log_lines if METRIC_MARKERS[m] in line]
        if len(metric_lines) > 0:
            time_metric_tuples = [convert_metric_line(
                line, METRIC_MARKERS[m], start_time) for line in metric_lines]
            result[m] = time_metric_tuples
    return result


def convert_single_timeseries(metric_map: dict[str, list[tuple]]) -> list[tuple[int, dict[str, int]]]:
    """
    convert from
    metric -> list of (timestamp, value)
    to 
    list of (timestamp, (metric -> value))
    """
    metric_values = {}
    timestamps = None
    for metric in metric_map.keys():
        # timestamps are same for all metrics with ms accuracy
        if timestamps is None:
            timestamps = [tpl[0] for tpl in metric_map[metric]]
        values = [tpl[1] for tpl in metric_map[metric]]
        metric_values[metric] = values
    result = list()
    for i, ts in enumerate(timestamps):
        result.append((ts, {m: metric_values[m][i]
                            for m in metric_map.keys()}))
    return result


def parse_to_timeseries(log_path: str) -> list[tuple[int, dict[str, int]]]:
    return convert_single_timeseries(parse_result(log_path))
