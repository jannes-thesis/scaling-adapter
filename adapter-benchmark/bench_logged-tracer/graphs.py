from typing import Union
from matplotlib import pyplot
from matplotlib.axes import Axes
import numpy


number = Union[float, int]

def get_avgd_over_x(xys: list[tuple[int, float]], interval_ms: int) -> list[tuple[int, float]]:
    last_timestamp = 0
    x_diffs = []
    last_x = 0
    last_ys = []
    xs = []
    ys = []
    for x, y in xys:
        x_diffs.append(x - last_x)
        last_ys.append(y)
        if x > last_timestamp + interval_ms:
            weighted_sum = sum([tpl[0] * tpl[1] for tpl in list(zip(last_ys, x_diffs))])
            duration = x - last_timestamp
            ys.append(weighted_sum / duration)
            xs.append(x)
            last_timestamp = x
            x_diffs = []
            last_ys = []
        last_x = x
    return list(zip(xs, ys))


def plot_timeseries_multiple(result: list[tuple[int, dict[str, number]]], save_path: str, title_suffix: str):
    amount_metrics = len(result[0][1].keys())
    fig, axs = pyplot.subplots(
        nrows=amount_metrics, figsize=(20, amount_metrics*10))
    xs = [tpl[0] for tpl in result]
    for i, metric in enumerate(result[0][1].keys()):
        ys = [tpl[1][metric] for tpl in result]
        axs[i].plot(xs, ys, 'r-')
        axs[i].set_xlabel('runtime in ms')
        axs[i].set_ylabel(f'{metric} {title_suffix}')
        axs[i].grid(color='grey', linestyle='-', linewidth=0.25, alpha=0.5)
        axs[i].set_title(f'{metric} {title_suffix} over time')
    fig.savefig(save_path)
    pyplot.close(fig)


def plot_rwchar_rate(derived: list[tuple[int, dict[str, number]]], 
                     absolute: list[tuple[int, dict[str, number]]], 
                     ax: Axes, workload: str, omit_change_points=False):
    xs = [tpl[0] / 1000 for tpl in derived]
    ys = [tpl[1]['rwchar/sec'] for tpl in derived]
    if 'psize' in absolute[0][1]:
        y2s = [tpl[1]['psize'] for tpl in absolute]
        ax2 = ax.twinx()
        ax2.plot(xs, y2s, 'b-')
        ax2.set_ylabel('pool size')
        if omit_change_points:
            last_pool_size = 0
            xs_filtered = list()
            ys_filtered = list()
            for i in range(len(xs)):
                if y2s[i] != last_pool_size:
                    last_pool_size = y2s[i]
                else:
                    xs_filtered.append(xs[i])
                    ys_filtered.append(ys[i])
            xs = xs_filtered
            ys = ys_filtered
    ax3 = ax.twinx()
    xy3s = get_avgd_over_x(list(zip([tpl[0] for tpl in derived], ys)), 5000)
    x3s = [tpl[0] / 1000 for tpl in xy3s]
    y3s = [tpl[1] for tpl in xy3s]
    ax3.plot(x3s, y3s, 'g-')
    # window_width = 51
    # cumsums = numpy.cumsum(numpy.insert(numpy.array(xs), 0, 0)) 
    # moving_averages = (cumsums[window_width:] - cumsums[:-window_width]) / window_width
    # y3s = ys[25:-25]
    # ax3.plot(moving_averages, y3s, 'g-')
    workload = workload.split('_')[-1]
    ax.plot(xs, ys, 'r-')
    ax.set_xlabel('runtime in seconds')
    ax.set_ylabel(f'rwchar/sec')
    ax.grid(color='grey', linestyle='-', linewidth=0.25, alpha=0.5)
    ax.set_title(f'rwchar/sec {workload}')


def plot_iosyscalls_rate(rates: list[tuple[int, dict[str, number]]], 
                         absolute: list[tuple[int, dict[str, number]]], 
                         ax: Axes, workload: str):
    xs = [tpl[0] / 1000 for tpl in rates]
    ys = [tpl[1]['syscall_count'] for tpl in rates]
    if 'psize' in absolute[0][1]:
        y2s = [tpl[1]['psize'] for tpl in absolute]
        ax2 = ax.twinx()
        ax2.plot(xs, y2s, 'b-')
        ax2.set_ylabel('pool size')
    workload = workload.split('_')[-1]
    ax.plot(xs, ys, 'r-')
    ax.set_xlabel('runtime in seconds')
    ax.set_ylabel(f'iosyscalls/sec')
    ax.grid(color='grey', linestyle='-', linewidth=0.25, alpha=0.5)
    ax.set_title(f'iosyscalls/sec {workload}')


def plot_iosyscalls_calltime(derived: list[tuple[int, dict[str, number]]], 
                             absolute: list[tuple[int, dict[str, number]]], 
                             ax: Axes, workload: str):
    xs = [tpl[0] / 1000 for tpl in derived]
    ys = [tpl[1]['iosysctime/#iosyscalls'] for tpl in derived]
    if 'psize' in absolute[0][1]:
        y2s = [tpl[1]['psize'] for tpl in absolute]
        ax2 = ax.twinx()
        ax2.plot(xs, y2s, 'b-')
        ax2.set_ylabel('pool size')
    workload = workload.split('_')[-1]
    ax.plot(xs, ys, 'r-')
    ax.set_xlabel('runtime in seconds')
    ax.set_ylabel(f'iosyscalls time/call in ms')
    ax.grid(color='grey', linestyle='-', linewidth=0.25, alpha=0.5)
    ax.set_title(f'iosyscalls avg calltime {workload}')
