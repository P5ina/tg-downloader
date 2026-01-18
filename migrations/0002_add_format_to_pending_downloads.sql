-- Add format column to pending_downloads for storing selected format before quality selection
ALTER TABLE pending_downloads ADD COLUMN format TEXT;
