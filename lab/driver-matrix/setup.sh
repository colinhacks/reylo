#!/usr/bin/env bash
# Starts the matrix's servers idempotently and seeds their fixture tables.
#
#   Postgres 14 (shared, local)  postgres://localhost:5432/postgres, schema driver_matrix only
#   porg-dm-mysql84              MySQL 8.4    127.0.0.1:33406  root/porg  database driver_matrix
#   porg-dm-mysql80              MySQL 8.0    127.0.0.1:33380  root/porg  database driver_matrix
#   porg-dm-neon-pg              Postgres 14  127.0.0.1:55414  postgres/postgres  database main (behind the Neon proxy)
#   porg-dm-neon-proxy           Neon proxy   127.0.0.1:54444  HTTP /sql and WebSocket /v1
#   porg-dm-ps-sim               PlanetScale HTTP API simulator  127.0.0.1:38080, in front of porg-dm-mysql84
#
# The Neon proxy creates a neon_control_plane schema in the database it fronts, so it gets a
# Postgres container of its own instead of the shared server.
set -euo pipefail
cd "$(dirname "$0")"

PG_URL="${DM_PG_URL:-postgres://localhost:5432/postgres}"
NET=porg-dm-net

docker info > /dev/null 2>&1 || { echo "setup.sh: Docker is not running" >&2; exit 1; }
docker network inspect "$NET" > /dev/null 2>&1 || docker network create "$NET" > /dev/null

# start NAME IMAGE [docker run args...] -- [image args...]
start() {
  local name=$1 image=$2
  shift 2
  local state
  if ! state=$(docker inspect -f '{{.State.Status}}' "$name" 2> /dev/null); then state=missing; fi
  case "$state" in
    running) ;;
    missing) docker run -d --name "$name" --network "$NET" "$@" > /dev/null; echo "started $name ($image)" ;;
    *) docker start "$name" > /dev/null; echo "restarted $name" ;;
  esac
}

# ps-http-sim keeps a MySQL connection open for every session it opens, about 70 for each
# PlanetScale job, so the server it fronts takes more than MySQL's default 151 connections.
start porg-dm-mysql84 mysql:8.4 -e MYSQL_ROOT_PASSWORD=porg -e MYSQL_DATABASE=driver_matrix -p 127.0.0.1:33406:3306 mysql:8.4 --max-connections=1000
start porg-dm-mysql80 mysql:8.0 -e MYSQL_ROOT_PASSWORD=porg -e MYSQL_DATABASE=driver_matrix -p 127.0.0.1:33380:3306 mysql:8.0
start porg-dm-neon-pg postgres:14 -e POSTGRES_PASSWORD=postgres -e POSTGRES_DB=main -p 127.0.0.1:55414:5432 postgres:14

wait_for() {
  local what=$1
  shift
  for _ in $(seq 1 120); do
    if "$@" > /dev/null 2>&1; then return 0; fi
    sleep 1
  done
  echo "setup.sh: $what did not become ready" >&2
  return 1
}

wait_for "MySQL 8.4" docker exec porg-dm-mysql84 mysql -uroot -pporg -h127.0.0.1 -e 'select 1'
wait_for "MySQL 8.0" docker exec porg-dm-mysql80 mysql -uroot -pporg -h127.0.0.1 -e 'select 1'
wait_for "Neon's Postgres" docker exec porg-dm-neon-pg pg_isready -U postgres -h 127.0.0.1

# The proxy and the simulator need their upstream to be up before they start.
start porg-dm-neon-proxy ghcr.io/timowilhelm/local-neon-http-proxy:main -e PG_CONNECTION_STRING=postgres://postgres:postgres@porg-dm-neon-pg:5432/main -p 127.0.0.1:54444:4444 ghcr.io/timowilhelm/local-neon-http-proxy:main
start porg-dm-ps-sim ghcr.io/mattrobenolt/ps-http-sim:latest -p 127.0.0.1:38080:8080 ghcr.io/mattrobenolt/ps-http-sim:latest \
  -listen-addr=0.0.0.0 -listen-port=8080 -mysql-addr=porg-dm-mysql84 -mysql-port=3306 -mysql-dbname=driver_matrix -log-level=warn

mkdir -p .out
# seed LABEL COMMAND...: runs the command with its output in a log, and stops on a failure.
seed() {
  local label=$1
  shift
  if "$@" > .out/seed.log 2>&1; then echo "seeded $label"; else echo "setup.sh: seeding $label failed:" >&2; cat .out/seed.log >&2; exit 1; fi
}
seed "MySQL 8.4" sh -c 'docker exec -i porg-dm-mysql84 mysql -uroot -pporg < seed/mysql.sql'
seed "MySQL 8.0" sh -c 'docker exec -i porg-dm-mysql80 mysql -uroot -pporg < seed/mysql.sql'
seed "the shared Postgres (schema driver_matrix)" psql "$PG_URL" -q -v ON_ERROR_STOP=1 -f seed/postgres.sql
seed "Neon's Postgres" sh -c 'docker exec -i porg-dm-neon-pg psql -q -v ON_ERROR_STOP=1 -U postgres -d main < seed/postgres.sql'
echo "setup.sh: ready"
