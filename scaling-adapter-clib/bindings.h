#include <cstdarg>
#include <cstdint>
#include <cstdlib>
#include <new>

struct IntervalMetricsFFI {
  double scale_metric;
  double idle_metric;
  uint32_t current_nr_targets;
};

struct IntervalDataFFI {
  uint64_t read_bytes;
  uint64_t write_bytes;
  const SyscallData *syscalls_data;
};

using CalcMetricsFunFFI = IntervalMetricsFFI(*)(const IntervalDataFFI*);

extern "C" {

bool new_adapter(uint64_t check_interval_ms,
                 const int32_t *syscall_nrs,
                 uintptr_t amount_syscalls,
                 CalcMetricsFunFFI calc_interval_metrics);

} // extern "C"
