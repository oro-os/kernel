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

$CC -Wall -Wextra -Werror -Wno-fixed-enum-extension -std=c99 -pedantic -o /dev/null -x c - <<EOF
#include "$PTH"
int main(void) { return 0; }
EOF

$CXX -Wall -Wextra -Werror -std=c++11 -pedantic -o /dev/null -x c++ - <<EOF
#include "$PTH"
int main() { return 0; }
EOF

set +x

echo
echo "ORO BOOT PROTOCOL HEADER OK"
