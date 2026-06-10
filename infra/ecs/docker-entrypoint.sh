#!/bin/sh
# First boot of a fresh task: seed the demo control plane (principals, root
# catalog, Biscuit capabilities) and run one offline-floor ingest so the brain
# answers queries immediately. Subsequent restarts keep existing state.
set -eu

if [ ! -d "${CONTEXTFUL_HOME:?}/control" ]; then
    echo "seeding control plane into ${CONTEXTFUL_HOME}" >&2
    sync ctl seed
    sync ingest --source stripe || echo "ingest failed; serving anyway" >&2
fi

exec sync "$@"
