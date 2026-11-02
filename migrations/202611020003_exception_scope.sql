ALTER TABLE exceptions ADD COLUMN access_mode text NOT NULL DEFAULT 'read' CHECK(access_mode IN ('read','write'));
