import sys
from parse_log import parse_result 
from plot import plot_adapter_timeseries 


if __name__ == '__main__':
    log_name = sys.argv[1]
    out = sys.argv[2]
    res = parse_result(log_name)
    plot_adapter_timeseries(out, res)
