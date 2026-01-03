#!/bin/bash
LOGFILE="/tmp/ollama-cleanup.log"
INPUT=$(cat)

echo "=== $(date) ===" >> "$LOGFILE"
echo "INPUT: $INPUT" >> "$LOGFILE"

# Build JSON payload properly with jq
JSON=$(jq -n --arg text "$INPUT" '{
  model: "llama3.2:1b",
  prompt: ("Clean up this dictated text. Remove filler words (um, uh), fix grammar. Output ONLY the cleaned text, no quotes:\n\n" + $text),
  stream: false
}')

OUTPUT=$(curl -s http://localhost:11434/api/generate -d "$JSON" | jq -r '.response // empty' | sed 's/^"//;s/"$//' | tail -1)

echo "OUTPUT: $OUTPUT" >> "$LOGFILE"
echo "" >> "$LOGFILE"

echo "$OUTPUT"
