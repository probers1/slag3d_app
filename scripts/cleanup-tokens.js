#!/usr/bin/env node
/**
 * Cleans up expired authentication tokens from the database.
 * This script should be run periodically (e.g., via a cron job or scheduled task)
 * to prevent the `auth_tokens` table from growing indefinitely with invalid records.
 */

import 'dotenv/config';
import path from 'node:path';
import fs from 'node:fs';
import Database from 'better-sqlite3';

// --- Database Path Resolution ---
function resolveDbPath() {
  const configured = process.env.DB_PATH?.trim();
  if (configured) return path.resolve(configured);

  if (fs.existsSync('/data') && fs.statSync('/data').isDirectory()) {
    return path.join('/data', 'vault.db');
  }

  return path.resolve(process.cwd(), 'data', 'vault.db');
}

/**
 * Deletes tokens that have expired and are far enough in the past to be considered garbage.
 * @param {number} daysToKeep - Number of days to keep tokens for audit purposes.
 */
function cleanupExpiredTokens(daysToKeep) {
  const dbPath = resolveDbPath();
  console.log(`[Token Cleanup] Connecting to database at: ${dbPath}`);
  
  if (!fs.existsSync(dbPath)) {
    console.error('[Token Cleanup] ERROR: Database file not found. Did the application run first?');
    process.exit(1);
  }
  
  const db = new Database(dbPath);
  
  // Calculate the cutoff timestamp (e.g., 7 days ago)
  const cutoffDate = new Date();
  cutoffDate.setDate(cutoffDate.getDate() - daysToKeep);
  const cutoffTimestamp = cutoffDate.getTime();

  // SQL to delete records where 'expires_at' is less than the cutoff time.
  // Assuming 'expires_at' is the field name and it stores milliseconds since epoch.
  const sql = `DELETE FROM auth_tokens WHERE expires_at < ?;`;
  
  try {
    const stmt = db.prepare(sql);
    const result = stmt.run(cutoffTimestamp);
    
    console.log(`\n======================================================`);
    console.log(`✅ Token Cleanup Complete.`);
    console.log(`   Deleted ${result.changes} expired tokens from auth_tokens.`);
    console.log(`   Tokens older than ${new Date(cutoffTimestamp).toISOString()} were removed.`);
    console.log(`======================================================\n`);

  } catch (error) {
    console.error('[Token Cleanup] FATAL ERROR during token cleanup:', error);
  } finally {
    db.close();
  }
}

// --- Execution ---
// We keep tokens for 7 days for forensic/audit purposes, as suggested by the README.
const DAYS_TO_KEEP = 7; 
cleanupExpiredTokens(DAYS_TO_KEEP);