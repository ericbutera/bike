DELETE FROM heatmap_chunks c USING heatmap_projections p
WHERE c.activity_id=p.activity_id AND p.projection_version < 4;
UPDATE heatmap_user_states SET revision=revision+1
WHERE user_id IN (SELECT DISTINCT user_id FROM heatmap_projections WHERE projection_version < 4);
UPDATE heatmap_projections SET generation=generation+1, projection_version=4,
status='pending', queued_at=NULL, error=NULL, min_x=NULL, min_y=NULL, max_x=NULL, max_y=NULL
WHERE projection_version < 4;
ALTER TABLE heatmap_projections ALTER COLUMN projection_version SET DEFAULT 4;
-- An older worker may lease a fresh generation during rollout. Reject its entire
-- publication transaction instead of allowing old chunks to become renderable.
ALTER TABLE heatmap_projections ADD CONSTRAINT heatmap_current_recording_policy
CHECK (status NOT IN ('ready','skipped') OR projection_version >= 4);
