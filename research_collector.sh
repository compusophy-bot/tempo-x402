#!/bin/bash
PEERS=
echo '[]' > market_research_raw.json
for peer in ; do
  echo "Fetching from /endpoints"
  curl -s -m 5 "/endpoints" >> market_research_raw.json || echo 'failed' >> market_research_raw.json
done
