-- Preserve Web Scan producer identity and prompt separately from canonical work-item fields.
-- A non-NULL producer_check_id marks rows written under this contract.
ALTER TABLE work_items ADD COLUMN producer_check_id TEXT;
ALTER TABLE work_items ADD COLUMN producer_fix_prompt TEXT;
ALTER TABLE work_items ADD COLUMN producer_category TEXT
    CHECK (producer_category IS NULL OR producer_category IN
           ('security', 'performance', 'seo', 'accessibility', 'compliance', 'config', 'polish'));
