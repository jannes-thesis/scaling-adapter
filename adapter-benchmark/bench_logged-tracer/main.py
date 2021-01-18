import sys
import os
import shutil
from matplotlib import pyplot
from parse_log import parse_to_timeseries
from derived import timeseries_to_rate_all, timeseries_to_derived
from graphs import plot_timeseries_multiple, plot_rwchar_rate, plot_iosyscalls_rate, plot_iosyscalls_calltime


def single_plot(metric, output_name, log_names):
    if len(log_names) != 2:
        raise Exception('amount logs not equal to 2')
    fig, axs = pyplot.subplots(nrows=1, ncols=2, figsize=(30, 10))
    if 'rwchar-rate' in metric:
        if metric == 'rwchar-rate':
            omit = False
        elif metric == 'rwchar-rate-filtered':
            omit = True
        else:
            raise Exception(f'metric {metric} not supported')
        for i, log_name in enumerate(log_names):
            workload = log_name.split('.')[0]
            parsed = parse_to_timeseries(log_name)
            derived = timeseries_to_derived(parsed)
            plot_rwchar_rate(derived, parsed, axs[i], workload, omit)
        fig.tight_layout()
        fig.savefig(output_name)
        pyplot.close(fig)
    elif metric == 'iosyscalls-rate':
        for i, log_name in enumerate(log_names):
            workload = log_name.split('.')[0]
            parsed = parse_to_timeseries(log_name)
            original_rates = timeseries_to_rate_all(parsed)
            plot_iosyscalls_rate(original_rates, parsed, axs[i], workload)
        fig.tight_layout()
        fig.savefig(output_name)
        pyplot.close(fig)
    elif metric == 'iosyscalls-calltime':
        for i, log_name in enumerate(log_names):
            workload = log_name.split('.')[0]
            parsed = parse_to_timeseries(log_name)
            derived = timeseries_to_derived(parsed)
            plot_iosyscalls_calltime(derived, parsed, axs[i], workload)
        fig.tight_layout()
        fig.savefig(output_name)
        pyplot.close(fig)
    else: 
        raise Exception(f'metric {metric} not supported')


if __name__ == '__main__':
    if sys.argv[1] == '--single':
        metric = sys.argv[2]
        output_name = sys.argv[3]
        log_names = sys.argv[4:]
        single_plot(metric, output_name, log_names)
    else:
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
