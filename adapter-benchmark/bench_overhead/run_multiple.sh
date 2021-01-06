#!/bin/bash
# ./run.sh oneshot read_write_4kb_sync 1000 os-rw4kb-1k
./run.sh oneshot read_write_4kb_sync 100000 os-rw4kb-100k
./run.sh every100us read_write_4kb_sync 100000 100us-rw4kb-10k
./run.sh oneshot read_write_buf_sync_1mb 1000 os-rwbuf1mb-1k
./run.sh every100ms read_write_buf_sync_1mb 1000 100ms-rwbuf1mb-1k
