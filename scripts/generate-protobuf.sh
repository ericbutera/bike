#!/bin/sh
set -eu

case "${1:-}" in
  '') check=false ;;
  --check) check=true ;;
  *) echo 'usage: generate-protobuf.sh [--check]' >&2; exit 2 ;;
esac

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"
export GOPATH="$repo_root/.cache/go-path"
export GOMODCACHE="$repo_root/.cache/go-mod"
export GOCACHE="$repo_root/.cache/go-build"
export GOBIN="$repo_root/.cache/proto-bin"
mkdir -p "$GOBIN"

go install google.golang.org/protobuf/cmd/protoc-gen-go@v1.36.6
go install google.golang.org/grpc/cmd/protoc-gen-go-grpc@v1.5.1

output_dir=$(mktemp -d)
trap 'rm -rf "$output_dir"' EXIT
protoc -I proto \
  --go_out="$output_dir" \
  --go_opt=module=github.com/ericbutera/bike-services/strava-gateway \
  --go-grpc_out="$output_dir" \
  --go-grpc_opt=module=github.com/ericbutera/bike-services/strava-gateway \
  --plugin=protoc-gen-go="$GOBIN/protoc-gen-go" \
  --plugin=protoc-gen-go-grpc="$GOBIN/protoc-gen-go-grpc" \
  proto/bike/strava/v1/gateway.proto

generated_dir=strava-gateway/gen/bike/strava/v1
for name in gateway.pb.go gateway_grpc.pb.go; do
  if [ "$check" = true ]; then
    cmp "$output_dir/gen/bike/strava/v1/$name" "$generated_dir/$name"
  else
    cp "$output_dir/gen/bike/strava/v1/$name" "$generated_dir/$name"
  fi
done
