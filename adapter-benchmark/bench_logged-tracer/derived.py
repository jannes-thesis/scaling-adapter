def timeseries_to_rate_all(timeseries: list[tuple[int, dict[str, int]]]) -> list[tuple[int, dict[str, float]]]:
    metrics = timeseries[0][1].keys()
    result = [(tpl[0], {}) for tpl in timeseries]
    for metric in metrics:
        rates = timeseries_to_rate([(tpl[0], tpl[1][metric]) for tpl in timeseries])
        for i in range(len(timeseries)):
            result[i][1][metric] = rates[i][1]
    return result


def timeseries_to_derived(timeseries: list[tuple[int, dict[str, int]]]) -> list[tuple[int, dict[str, float]]]:
    derive_functions = {
        'iowait*iosyctime/sec': timeseries_to_iowait_iosysctime_rate,
        'rwchar/sec': timeseries_to_rw_rate,
        'rwbytes/sec': timeseries_to_disk_rw_rate,
        'iosysctime/#iosyscalls': timeseries_to_avg_iosyscalltime
    }
    result = [(tpl[0], {}) for tpl in timeseries]
    for derive in derive_functions.keys():
        f = derive_functions[derive]
        vals = [tpl[1] for tpl in f(timeseries)]
        for i in range(len(timeseries)):
            result[i][1][derive] = vals[i]
    return result


def timeseries_to_rate(time_metric_tuples: list[tuple[int, int]]) -> list[tuple[int, float]]:
    result = list()
    last_val = 0
    last_time = 0
    for timestamp, val in time_metric_tuples:
        duration = timestamp - last_time
        val_diff = val - last_val
        result.append((timestamp, val_diff / duration * 1000))
        last_val = val
        last_time = timestamp
    return result


def timeseries_to_diffs_multiple(timeseries: list[tuple[int, dict[str, int]]]) -> list[tuple[int, dict[str, int]]]:
    metrics = timeseries[0][1].keys()
    result = [(tpl[0], {}) for tpl in timeseries]
    for metric in metrics:
        diffs = timeseries_to_diffs([(tpl[0], tpl[1][metric]) for tpl in timeseries])
        for i in range(len(timeseries)):
            result[i][1][metric] = diffs[i][1]
    return result


def timeseries_to_diffs(time_metric_tuples: list[tuple[int, int]]) -> list[tuple[int, int]]:
    result = list()
    last_val = 0
    last_time = 0
    for timestamp, val in time_metric_tuples:
        duration = timestamp - last_time
        val_diff = val - last_val
        result.append((duration, val_diff))
        last_val = val
        last_time = timestamp
    return result


def timeseries_to_iowait_iosysctime_rate(timeseries: list[tuple[int, dict[str, int]]]) -> list[tuple[int, float]]:
    timeseries_diffs = timeseries_to_diffs_multiple(timeseries)
    timestamps = [tpl[0] for tpl in timeseries]
    def iowait_times_iosysctime(metric_map: dict[str, int]) -> int:
        return metric_map['blkio_delay'] * metric_map['syscall_time']
    iowait_iosysc_rates = [iowait_times_iosysctime(tpl[1]) / tpl[0] for tpl in timeseries_diffs]
    return list(zip(timestamps, iowait_iosysc_rates))


def timeseries_to_rw_rate(timeseries: list[tuple[int, dict[str, int]]]) -> list[tuple[int, float]]:
    def rchar_plus_wchar(metric_map: dict[str, int]) -> int:
        return metric_map['rchar'] + metric_map['wchar']
    rw_sums = [(tpl[0], rchar_plus_wchar(tpl[1])) for tpl in timeseries]
    return timeseries_to_rate(rw_sums)


def timeseries_to_disk_rw_rate(timeseries: list[tuple[int, dict[str, int]]]) -> list[tuple[int, float]]:
    def rbytes_plus_wbytes(metric_map: dict[str, int]) -> int:
        return metric_map['rbytes'] + metric_map['wbytes']
    rw_sums = [(tpl[0], rbytes_plus_wbytes(tpl[1])) for tpl in timeseries]
    return timeseries_to_rate(rw_sums)


def timeseries_to_avg_iosyscalltime(timeseries: list[tuple[int, dict[str, int]]]) -> list[tuple[int, float]]:
    timeseries_diffs = timeseries_to_diffs_multiple(timeseries)
    return [(tpl[0], tpl[1]['syscall_time'] / tpl[1]['syscall_count']) for tpl in timeseries_diffs]