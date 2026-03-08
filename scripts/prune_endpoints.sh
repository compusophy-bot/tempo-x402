#!/bin/bash
# scripts/prune_endpoints.sh
#
# Prunes redundant script endpoints from /data/endpoints.
# Identifies endpoints to keep based on the SYSTEM_PRUNING_AUDIT.md 
# and the instruction to keep only core 'chat', 'soul', and 'info' services.
#
# Goal: >50% reduction.

SCRIPTS_DIR="/data/endpoints"

if [ ! -d "$SCRIPTS_DIR" ]; then
    echo "Error: $SCRIPTS_DIR not found. This script should be run in an environment where /data/endpoints exists."
    exit 1
fi

# Core scripts to KEEP
KEEP="active-plans.sh analyze-siblings.sh capability-audit.sh financial-audit.sh growth-metrics.sh identity.sh info.sh mission-report.sh revenue-sink.sh soul-summary.sh system-pulse.sh"

echo "--- Endpoint Pruning Audit ---"
initial_count=$(ls "$SCRIPTS_DIR"/*.sh 2>/dev/null | wc -l)
echo "Initial script count: $initial_count"

pruned_count=0
echo "Pruning non-core scripts..."

for script_path in "$SCRIPTS_DIR"/*.sh; do
    [ -f "$script_path" ] || continue
    script=$(basename "$script_path")
    
    case " $KEEP " in
        *" $script "*) ;;
        *) rm "$script_path"; ((pruned_count++)) ;;
    esac
done

final_count=$(ls "$SCRIPTS_DIR"/*.sh 2>/dev/null | wc -l)
echo "Final script count: $final_count"
echo "Pruned $pruned_count scripts."

if [ "$initial_count" -gt 0 ]; then
    reduction_pct=$((pruned_count * 100 / initial_count))
    echo "Reduction: $reduction_pct%"
    
    if [ "$reduction_pct" -ge 50 ]; then
        echo "SUCCESS: Achieved $reduction_pct% reduction (goal: >=50%)."
    else
        echo "WARNING: Only achieved $reduction_pct% reduction (goal: >=50%)."
    fi
else
    echo "No scripts found to prune."
fi

echo "--- Remaining Core Scripts ---"
ls "$SCRIPTS_DIR"
