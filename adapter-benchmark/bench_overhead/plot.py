import matplotlib.pyplot as plt
import numpy as np
import json

workloads = {'rw4kb300k': 'os-rw4kb-300k', 'rwbuf2mb2k': 'os-rwbuf2mb-2k',
             'read2mb30k': 'os-read2mb-30k', 'rw2mb10k': 'os-rw2mb-10k',
             'rw2mb-nosync20k': 'os-rwnosync2mb-20k'}

if __name__ == '__main__':
    workload_names = []
    fixed_means = []
    fixed_stddevs = []
    overhead_means = []
    overhead_stddevs = []
    for workload in workloads.keys():
        workload_names.append(workload)
        with open(f'{workloads[workload]}-fixed.json') as f:
            res = json.load(f)['results'][0]
        fixed_means.append(res['mean'])
        fixed_stddevs.append(res['stddev'])
        with open(f'{workloads[workload]}-overhead.json') as f:
            res = json.load(f)['results'][0]
        overhead_means.append(res['mean'])
        overhead_stddevs.append(res['stddev'])
    
    x = np.arange(len(workload_names))  # the label locations
    width = 0.35  # the width of the bars
    
    fig, ax = plt.subplots(figsize=(20, 10))
    rects1 = ax.bar(x - width/2, fixed_means, width, label='without', yerr=fixed_stddevs)
    rects2 = ax.bar(x + width/2, overhead_means, width, label='with', yerr=overhead_stddevs)
    
    # Add some text for labels, title and custom x-axis tick labels, etc.
    ax.set_ylabel('Runtime in seconds')
    ax.set_title('Runtimes without/with adapter')
    ax.set_xticks(x)
    ax.set_xticklabels(workload_names)
    ax.legend()
    ax.set_aspect('auto')
    
    max_stddev = max(max(overhead_stddevs), max(fixed_stddevs))
    min_mean = min(min(overhead_means), min(fixed_means))
    def autolabel(rects):
        """Attach a text label above each bar in *rects*, displaying its height."""
        for rect in rects:
            height = round(rect.get_height(), 2)
            ax.annotate('{}'.format(height),
                        xy=(rect.get_x() + rect.get_width() / 2, min_mean - max_stddev - 10),
                        xytext=(0, 0),
                        textcoords="offset points",
                        ha='center', va='bottom')
    
    
    autolabel(rects1)
    autolabel(rects2)
    fig.savefig('result3.png')