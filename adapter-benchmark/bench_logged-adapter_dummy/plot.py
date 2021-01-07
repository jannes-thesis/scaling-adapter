from matplotlib import pyplot
from matplotlib.axes import Axes 


# https://matplotlib.org/3.1.1/gallery/ticks_and_spines/multiple_yaxis_with_spines.html
def plot_adapter_timeseries(name: str, parsed: dict):
    
    fig, (axm1, axm1sd, axm2, axm2sd) = pyplot.subplots(nrows=4, figsize=(20, 40))
    ax_m1 = axm1.twinx()
    ax_m2 = axm2.twinx()
    ax_m1sd = axm1sd.twinx()
    ax_m2sd = axm2sd.twinx()

    psizes = parsed['pool_size']
    m1s = parsed['metric_one']
    m1sds = parsed['metric_one_sd']
    m2s = parsed['metric_two']
    m2sds = parsed['metric_two_sd']

    ts1, psizes = list(zip(*psizes))
    ts2, m1s = list(zip(*m1s))
    ts3, m1sds = list(zip(*m1sds))
    ts4, m2s = list(zip(*m2s))
    ts5, m2sds = list(zip(*m2sds))

    # Plot 1
    p1a, = axm1.plot(ts1, psizes, "b-", label="pool size")
    p1b, = ax_m1.plot(ts2, m1s, "r-", label="m1 mean")

    axm1.set_xlabel("time in millis")
    axm1.set_ylabel("pool size")
    ax_m1.set_ylabel("m1 mean")
    axm1.yaxis.label.set_color(p1a.get_color())
    ax_m1.yaxis.label.set_color(p1b.get_color())

    axm1.tick_params(axis='y', colors=p1a.get_color())
    ax_m1.tick_params(axis='y', colors=p1b.get_color())
    axm1.tick_params(axis='x')
    lines = [p1a, p1b]
    axm1.legend(lines, [l.get_label() for l in lines])

    # Plot 2
    p2a, = axm1sd.plot(ts1, psizes, "b-", label="pool size")
    p2b, = ax_m1sd.plot(ts3, m1sds, "r-", label="m1 stddev")

    axm1sd.set_xlabel("time in millis")
    axm1sd.set_ylabel("pool size")
    ax_m1sd.set_ylabel("m1 stddev")
    axm1sd.yaxis.label.set_color(p2a.get_color())
    ax_m1sd.yaxis.label.set_color(p2b.get_color())

    axm1sd.tick_params(axis='y', colors=p2a.get_color())
    ax_m1sd.tick_params(axis='y', colors=p2b.get_color())
    axm1sd.tick_params(axis='x')
    lines = [p2a, p2b]
    axm1sd.legend(lines, [l.get_label() for l in lines])

    # Plot 3
    p3a, = axm2.plot(ts1, psizes, "b-", label="pool size")
    p3b, = ax_m2.plot(ts4, m2s, "r-", label="m2 mean")

    axm2.set_xlabel("time in millis")
    axm2.set_ylabel("pool size")
    ax_m2.set_ylabel("m2 mean")
    axm2.yaxis.label.set_color(p3a.get_color())
    ax_m2.yaxis.label.set_color(p3b.get_color())

    axm2.tick_params(axis='y', colors=p3a.get_color())
    ax_m2.tick_params(axis='y', colors=p3b.get_color())
    axm2.tick_params(axis='x')
    lines = [p3a, p3b]
    axm2.legend(lines, [l.get_label() for l in lines])

    # Plot 4
    p4a, = axm2sd.plot(ts1, psizes, "b-", label="pool size")
    p4b, = ax_m2sd.plot(ts5, m2sds, "r-", label="m2 stddev")

    axm2sd.set_xlabel("time in millis")
    axm2sd.set_ylabel("pool size")
    ax_m2sd.set_ylabel("m2 stddev")
    axm2sd.yaxis.label.set_color(p4a.get_color())
    ax_m2sd.yaxis.label.set_color(p4b.get_color())

    axm2sd.tick_params(axis='y', colors=p4a.get_color())
    ax_m2sd.tick_params(axis='y', colors=p4b.get_color())
    axm2sd.tick_params(axis='x')
    lines = [p4a, p4b]
    axm2sd.legend(lines, [l.get_label() for l in lines])

    fig.savefig(name)