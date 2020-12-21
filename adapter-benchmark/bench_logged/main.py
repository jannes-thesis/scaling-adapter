import sys
import os
import shutil
from parse_log import parse_to_timeseries
from derived import timeseries_to_rate_all, timeseries_to_derived
from graphs import plot_timeseries_multiple


if __name__ == '__main__':
    log_names = sys.argv[1:]
    for log_name in log_names:
        workload = log_name.split('.')[0]
        if os.path.exists(workload):
            shutil.rmtree(workload)
        os.mkdir(workload)
        parsed = parse_to_timeseries(log_name)
        original_rates = timeseries_to_rate_all(parsed)
        derived = timeseries_to_derived(parsed)
        plot_timeseries_multiple(parsed, f'{workload}/original-absolute.png', 'total since start')
        plot_timeseries_multiple(original_rates, f'{workload}/original-rates.png', 'in units/sec')
        plot_timeseries_multiple(derived, f'{workload}/derived.png', '')
