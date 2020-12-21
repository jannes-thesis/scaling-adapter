from typing import Union
from matplotlib import pyplot


number = Union[float, int]


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
