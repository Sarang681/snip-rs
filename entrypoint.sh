#!/bin/sh

set -ex

echo "Running SQLX migrations...."
sqlx migrate run

echo "Executing the application binary"
exec ./snip-rs
