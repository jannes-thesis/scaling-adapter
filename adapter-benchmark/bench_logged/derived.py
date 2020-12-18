def absolute_to_rate_all(timeseries: list[tuple[int, dict[str, int]]]) -> list[tuple[int, dict[str, float]]]:
    metrics = timeseries[0][1].keys()
    result = [(tpl[0], {}) for tpl in timeseries]
    for metric in metrics:
        rates = absolute_to_rate([(tpl[0], tpl[1][metric]) for tpl in timeseries])
        for i in range(len(timeseries)):
            result[i][1][metric] = rates[i][1]
    return result


def absolute_to_rate(time_metric_tuples: list[tuple[int, int]]) -> list[tuple[int, float]]:
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
