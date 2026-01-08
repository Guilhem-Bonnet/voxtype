#!/bin/bash
# Example post-processing script for Voxtype using OpenAI API
#
# Usage: Configure in ~/.config/voxtype/config.toml:
#   [output.post_process]
#   command = "/path/to/openai-cleanup.sh"
#   timeout_ms = 10000
#
# Requirements:
# - OPENAI_API_KEY environment variable set
# - curl and jq installed
#
# Tips:
# - gpt-4o-mini is fast and cheap for this use case
# - The prompt explicitly says "no emojis" because ydotool can't type them

set -euo pipefail

if [[ -z "${OPENAI_API_KEY:-}" ]]; then
    echo "Error: OPENAI_API_KEY not set" >&2
    cat  # Pass through original text on error
    exit 0
fi

INPUT=$(cat)

# Empty input = empty output
if [[ -z "$INPUT" ]]; then
    exit 0
fi

# Build JSON payload with jq to handle special characters
JSON=$(jq -n --arg text "$INPUT" '{
  model: "gpt-4o-mini",
  messages: [
    {
      role: "system",
      content: "You clean up dictated text. Remove filler words (um, uh, like), fix grammar and punctuation. Output ONLY the cleaned text - no quotes, no emojis, no explanations."
    },
    {
      role: "user",
      content: $text
    }
  ],
  max_tokens: 1000
}')

# Call OpenAI API
RESPONSE=$(curl -s --max-time 8 \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $OPENAI_API_KEY" \
    -d "$JSON" \
    https://api.openai.com/v1/chat/completions)

# Extract the response text
OUTPUT=$(echo "$RESPONSE" | jq -r '.choices[0].message.content // empty')

if [[ -n "$OUTPUT" ]]; then
    echo "$OUTPUT"
else
    # On error, pass through original text
    echo "$INPUT"
fi
