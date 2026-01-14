-- Add git_branch column to sessions table
ALTER TABLE sessions ADD COLUMN git_branch VARCHAR(255);
