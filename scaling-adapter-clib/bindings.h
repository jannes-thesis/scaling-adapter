#ifndef scaling_adapter_h
#define scaling_adapter_h

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct {
  double scale_metric;
  double idle_metric;
  uint32_t current_nr_targets;
} IntervalMetricsFFI;

typedef struct {
  uint32_t count;
  uint64_t total_time;
} SyscallData;

typedef struct {
  uint64_t read_bytes;
  uint64_t write_bytes;
  const SyscallData *syscalls_data;
} IntervalDataFFI;

typedef IntervalMetricsFFI (*CalcMetricsFunFFI)(const IntervalDataFFI*);

bool new_adapter(uint64_t check_interval_ms,
                 const int32_t *syscall_nrs,
                 uintptr_t amount_syscalls,
                 CalcMetricsFunFFI calc_interval_metrics);

#endif /* scaling_adapter_h */
