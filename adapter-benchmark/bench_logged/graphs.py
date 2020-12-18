from matplotlib import pyplot


def plot_metric_rates(result: list[tuple[int, dict[str, int]]], save_path: str):
    amount_metrics = len(result[0][1].keys())
    fig, axs = pyplot.subplots(
        nrows=amount_metrics, figsize=(20, amount_metrics*10))
    xs = [tpl[0] for tpl in result]
    for i, metric in enumerate(result[0][1].keys()):
        ys = [tpl[1][metric] for tpl in result]
        axs[i].plot(xs, ys, 'r-')
        axs[i].set_xlabel('runtime in ms')
        axs[i].set_ylabel(f'{metric} in unit/s')
        axs[i].grid(color='grey', linestyle='-', linewidth=0.25, alpha=0.5)
        axs[i].set_title(metric)
    fig.savefig(save_path)
    pyplot.close(fig)