#!/bin/bash
# previous runs were all with pool size 16
# ./run.sh oneshot read_write_4kb_sync 1000 os-rw4kb-1k
# ./run.sh oneshot read_write_4kb_sync 100000 os-rw4kb-100k
# ./run.sh every100us read_write_4kb_sync 100000 100us-rw4kb-10k
# ./run.sh oneshot read_write_buf_sync_1mb 1000 os-rwbuf1mb-1k
# ./run.sh every100ms read_write_buf_sync_1mb 1000 100ms-rwbuf1mb-1k
# ./run.sh oneshot read_write_buf_sync_2mb 2000 os-rwbuf2mb-2k 32
# ./run.sh oneshot read_2mb 30000 os-read2mb-30k 16  
./run.sh oneshot read_write_2mb_sync 10000 os-rw2mb-10k 32
./run.sh oneshot read_write_2mb_nosync 20000 os-rwnosync2mb-20k 32
