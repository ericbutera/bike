#!/bin/sh
set -eu

for database in temporal temporal_visibility; do
  export SQL_DATABASE="$database"
  schema=temporal
  if [ "$database" = temporal_visibility ]; then
    schema=visibility
  fi
  temporal-sql-tool setup-schema -v 0.0
  temporal-sql-tool update-schema -d "/etc/temporal/schema/postgresql/v12/$schema/versioned"
done
