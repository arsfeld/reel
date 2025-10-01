#!/usr/bin/env bash
set -euo pipefail

TASK_ID=310
MAX_ITERATIONS=50
ITERATION=0

echo "Starting automated iterations for task-$TASK_ID"
echo "Maximum iterations: $MAX_ITERATIONS"
echo "================================================"

while [ $ITERATION -lt $MAX_ITERATIONS ]; do
    ITERATION=$((ITERATION + 1))
    echo ""
    echo "Iteration $ITERATION/$MAX_ITERATIONS"
    echo "-----------------------------------"

    # Check current task status
    TASK_STATUS=$(backlog task $TASK_ID --plain | grep -A1 "Status:" | tail -n1 | xargs || echo "unknown")
    echo "Current status: $TASK_STATUS"

    # Check if task is done
    if [[ "$TASK_STATUS" == "Done" ]]; then
        echo ""
        echo "✅ Task $TASK_ID is marked as Done!"
        echo "Completed in $ITERATION iterations"
        exit 0
    fi

    # Run Claude to work on the task
    echo "Running Claude to continue work on task $TASK_ID..."
    claude --verbose --dangerously-skip-permissions -p "Continue working on task $TASK_ID. Run 'nix develop -c cargo check' to see remaining warnings, then fix the next batch of warnings. IMPORTANT: The build must pass at all times - verify with 'nix develop -c cargo check' after each change. Check acceptance criteria as you complete them. When all warnings are fixed and all ACs are checked, mark the task as Done."

    # Brief pause between iterations
    sleep 2
done

echo ""
echo "⚠️  Reached maximum iterations ($MAX_ITERATIONS) without completing task $TASK_ID"
echo "Current status: $TASK_STATUS"
exit 1
