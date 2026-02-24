#!/bin/bash
# backup_db.sh - Backup automatico database ClearURLsBot

set -e

DB_PATH="bot.db"
BACKUP_DIR="backup"
DATE=$(date +%Y-%m-%d_%H-%M-%S)

mkdir -p "$BACKUP_DIR"
cp "$DB_PATH" "$BACKUP_DIR/clearurlsbot_$DATE.sqlite"
echo "Backup eseguito: $BACKUP_DIR/clearurlsbot_$DATE.sqlite"
