import { db } from '../db.ts';

export function getTagsWithCounts() {
  // Single LEFT JOIN + GROUP BY — the previous version issued one COUNT per
  // tag (N+1), which is fine at a few dozen tags and painful at hundreds.
  return db.prepare(`
    SELECT t.id, t.name, COUNT(mt.model_id) as count
    FROM tags t
    LEFT JOIN model_tags mt ON mt.tag_id = t.id
    GROUP BY t.id, t.name
    HAVING count > 0
    ORDER BY count DESC, t.name ASC
  `).all();
}
