#!/bin/bash
lines=`awk 'BEGIN { print "#include <sys/syscall.h>" } /p_syscall_meta/ { syscall = substr($NF, 19); printf "syscalls[SYS_%s] = \"%s\";\n", syscall, syscall }' /proc/kallsyms | sort -u | gcc -E -P - | grep traceset`
nr_lines=`echo "$lines" | wc -l`
if [[ $nr_lines -eq 2 ]]; then
    echo "yes"
else
    echo "no"
fi