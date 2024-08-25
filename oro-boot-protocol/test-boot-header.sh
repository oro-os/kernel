#!/usr/bin/env bash
set -euo pipefail

PTH="${1:-"${CARGO_TARGET_DIR:-./target}/oro-boot.h"}"

if [ -z "${PTH:-}" ]; then
	echo "usage: $0 <path-to-boot-header>"
	exit 2
fi

if [ ! -f "$PTH" ]; then
	echo "error: file not found: $PTH"
	exit 1
fi

CC=${CC:-cc}
CXX=${CXX:-c++}

echo "testing Oro boot protocol header: $PTH"
echo

set -x
$CC --version
$CXX --version

$CC -Wall -Wextra -Werror -std=c99 -pedantic -o /dev/null -x c - <<EOF
#include "$PTH"
int main(void) {
	oro_kernel_settings_data_v0_t data;
	data.linear_map_offset = 1234;
	(void)data;
	oro_memory_map_entry_t entry;
	entry.base = 1234;
	entry.length = 5678;
	entry.ty = ORO_BOOT_MEMORY_MAP_ENTRY_TYPE_USABLE;
	(void)entry;
	return 0;
}
EOF

$CXX -Wall -Wextra -Werror -std=c++11 -pedantic -o /dev/null -x c++ - <<EOF
#include "$PTH"
int main() {
	oro_boot::oro_kernel_settings_data_v0_t data;
	data.linear_map_offset = 1234;
	(void)data;
	oro_boot::oro_memory_map_entry_t entry;
	entry.base = 1234;
	entry.length = 5678;
	entry.ty = oro_boot::oro_memory_map_entry_type_t::USABLE;
	(void)entry;
	return 0;
}
EOF

set +x

echo
echo "ORO BOOT PROTOCOL HEADER OK"
