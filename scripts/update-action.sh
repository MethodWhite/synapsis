#!/bin/bash
# Usage: ./scripts/update-action.sh <action-name> <new-version>
# Example: ./scripts/update-action.sh actions/labeler v6
# This updates ALL workflow files that use the given action.

ACTION="$1"
NEW_VERSION="$2"

if [ -z "$ACTION" ] || [ -z "$NEW_VERSION" ]; then
  echo "Usage: $0 <action-name> <new-version>"
  echo "Example: $0 actions/labeler v6"
  exit 1
fi

# Remove trailing @version if present
ACTION_NAME="${ACTION%@*}"
echo "Updating $ACTION_NAME to @$NEW_VERSION in all workflows..."

for f in .github/workflows/*.yml; do
  if grep -q "uses: $ACTION_NAME@" "$f"; then
    sed -i "s|uses: $ACTION_NAME@.*|uses: $ACTION_NAME@$NEW_VERSION|" "$f"
    echo "  Updated $f"
  fi
done
