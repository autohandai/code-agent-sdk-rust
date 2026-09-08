#!/bin/sh
while IFS= read -r line; do
  printf '%s\n' "$line" >> "$AUTOHAND_TEST_LOG"
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *autohand.getState*) response='{}' ;;
    *autohand.prompt*)
      if [ -n "$AUTOHAND_TEST_NOTIFICATION" ]; then
        printf '%s\n' "$AUTOHAND_TEST_NOTIFICATION"
        printf '%s\n' '{"jsonrpc":"2.0","method":"autohand.agentEnd","params":{"sessionId":"fixture","reason":"completed","timestamp":"now"}}'
      fi
      response='{"success":true}'
      ;;
    *) response="$AUTOHAND_TEST_RESULT" ;;
  esac
  printf '{"jsonrpc":"2.0","id":%s,"result":%s}\n' "$id" "$response"
done
