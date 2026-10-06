ALTER TABLE heatmap_projections DROP CONSTRAINT heatmap_current_recording_policy;
DELETE FROM heatmap_chunks;
UPDATE heatmap_user_states SET revision=revision+1;
UPDATE heatmap_projections SET generation=generation+1, projection_version=5,
status='pending', queued_at=NULL, error=NULL, min_x=NULL, min_y=NULL, max_x=NULL, max_y=NULL;
ALTER TABLE heatmap_projections ALTER COLUMN projection_version SET DEFAULT 5;
ALTER TABLE heatmap_projections ADD CONSTRAINT heatmap_current_recording_policy
CHECK (status NOT IN ('ready','skipped') OR projection_version >= 5);
